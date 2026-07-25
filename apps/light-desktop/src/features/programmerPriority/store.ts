import {
	assertDuplicatePriorityChange,
	assertNextPriorityChange,
	priorityChangeUserId,
	priorityProtocolError,
} from "./change";
import type {
	ProgrammerPriorityChange,
	ProgrammerPriorityProjection,
	ProgrammerPrioritySnapshot,
} from "./contracts";
import {
	type OptimisticPriority,
	renderOptimisticPriority,
} from "./optimisticProjection";
import {
	assertPriorityCursor,
	assertProgrammerPriority,
	canonicalPriorityProjection,
	samePriorityProjection,
} from "./projectionValue";
import {
	emptyProgrammerPriorityState,
	type ProgrammerPrioritySettlement,
	type ProgrammerPriorityState,
} from "./storeState";
import { ProgrammerPriorityProtocolError } from "./transport";

export type {
	ProgrammerPrioritySettlement,
	ProgrammerPriorityState,
	ProgrammerPriorityStatus,
} from "./storeState";

export class ProgrammerPriorityStore {
	private readonly listeners = new Set<() => void>();
	private readonly operations = new Map<string, OptimisticPriority>();
	private authoritative: ProgrammerPriorityProjection | null = null;
	private scope = 0;
	private state = emptyProgrammerPriorityState();

	readonly subscribe = (listener: () => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};

	readonly getSnapshot = () => this.state;

	reset(userId: string | null, authorityKey = "") {
		if (
			userId === this.state.userId &&
			authorityKey === this.state.authorityKey
		)
			return;
		this.scope++;
		this.authoritative = null;
		this.operations.clear();
		this.state = { ...emptyProgrammerPriorityState(), userId, authorityKey };
		this.emit();
	}

	installSnapshot(
		snapshot: ProgrammerPrioritySnapshot,
		expectedScope = this.scope,
	) {
		return this.installSnapshotValue(snapshot, expectedScope, false);
	}

	installRepairSnapshot(
		snapshot: ProgrammerPrioritySnapshot,
		expectedScope = this.scope,
	) {
		return this.installSnapshotValue(snapshot, expectedScope, true);
	}

	applyChange(
		change: ProgrammerPriorityChange,
		sequence: number,
		expectedScope = this.scope,
	) {
		if (!this.canAccept(priorityChangeUserId(change), expectedScope))
			return false;
		try {
			assertPriorityCursor(sequence);
			const currentSequence = this.requireEventSequence(sequence);
			if (sequence < currentSequence) return true;
			if (sequence === currentSequence)
				return this.acceptDuplicate(change, sequence);
			this.assertNextChange(change, sequence);
			if (change.type === "remove") this.publishTombstone(change, sequence);
			else
				this.publishAuthority(
					canonicalPriorityProjection(change.projection),
					sequence,
				);
			return true;
		} catch (reason) {
			throw this.rejectProtocol(reason, sequence);
		}
	}

	beginOptimistic(
		requestId: string,
		priority: number,
		expectedScope = this.scope,
	) {
		if (
			!requestId ||
			!this.isScopeCurrent(expectedScope) ||
			!this.authoritative ||
			this.state.status !== "ready" ||
			this.state.repairRequired
		)
			return false;
		if (this.operations.has(requestId))
			throw new Error(
				`Programmer priority request ${requestId} is already pending`,
			);
		assertProgrammerPriority(priority);
		this.operations.set(requestId, { requestId, priority });
		this.publishRendered();
		return true;
	}

	settleChanged(
		requestId: string,
		projection: ProgrammerPriorityProjection,
		sequence: number,
		expectedScope = this.scope,
	): ProgrammerPrioritySettlement {
		if (!this.hasOperation(requestId, expectedScope)) return "ignored";
		try {
			if (!this.matchesUser(projection.userId)) return "ignored";
			this.applyChange({ type: "upsert", projection }, sequence, expectedScope);
			this.operations.delete(requestId);
			this.publishRendered();
			return "settled";
		} catch {
			return "repair";
		}
	}

	settleNoChange(
		requestId: string,
		projection: ProgrammerPriorityProjection,
		expectedScope = this.scope,
	): ProgrammerPrioritySettlement {
		if (!this.hasOperation(requestId, expectedScope)) return "ignored";
		try {
			const incoming = canonicalPriorityProjection(projection);
			if (!this.matchesUser(incoming.userId)) return "ignored";
			const currentRevision = this.state.authorityRevision;
			if (currentRevision === null || incoming.revision > currentRevision)
				return "repair";
			if (
				incoming.revision === currentRevision &&
				(!this.authoritative ||
					!samePriorityProjection(this.authoritative, incoming))
			)
				return "repair";
			this.operations.delete(requestId);
			this.publishRendered();
			return "settled";
		} catch (reason) {
			this.markProtocolRepair(reason, this.state.eventSequence);
			return "repair";
		}
	}

	rollback(requestId: string, error: Error, expectedScope = this.scope) {
		if (!this.hasOperation(requestId, expectedScope)) return false;
		this.operations.delete(requestId);
		this.publishRendered({ error });
		return true;
	}

	abandon(requestId: string, expectedScope = this.scope) {
		if (!this.hasOperation(requestId, expectedScope)) return false;
		this.operations.delete(requestId);
		this.publishRendered();
		return true;
	}

	setLoading(expectedScope = this.scope) {
		return this.publishState({ status: "loading", error: null }, expectedScope);
	}

	setReady(expectedScope = this.scope) {
		return this.publishState(
			{ status: "ready", error: null, repairRequired: false },
			expectedScope,
		);
	}

	setError(error: Error, expectedScope = this.scope) {
		return this.publishState({ status: "error", error }, expectedScope);
	}

	setRepairRequired(error: Error, expectedScope = this.scope) {
		return this.publishState(
			{ status: "error", error, repairRequired: true },
			expectedScope,
		);
	}

	captureScope() {
		return this.scope;
	}

	isScopeCurrent(scope: number) {
		return scope === this.scope;
	}

	authoritativeRevision(expectedScope = this.scope) {
		return this.isScopeCurrent(expectedScope)
			? this.state.authorityRevision
			: null;
	}

	hasOperation(requestId: string, expectedScope = this.scope) {
		return this.isScopeCurrent(expectedScope) && this.operations.has(requestId);
	}

	private installSnapshotValue(
		snapshot: ProgrammerPrioritySnapshot,
		expectedScope: number,
		repair: boolean,
	) {
		if (!this.canAccept(snapshot.projection.userId, expectedScope))
			return false;
		try {
			assertPriorityCursor(snapshot.cursor);
			const projection = canonicalPriorityProjection(snapshot.projection);
			if (repair) this.assertRepairDoesNotRegress(snapshot, projection);
			this.publishAuthority(projection, snapshot.cursor);
			return true;
		} catch (reason) {
			throw this.rejectProtocol(reason, snapshot.cursor);
		}
	}

	private assertRepairDoesNotRegress(
		snapshot: ProgrammerPrioritySnapshot,
		projection: ProgrammerPriorityProjection,
	) {
		if (
			this.state.eventSequence !== null &&
			snapshot.cursor < this.state.eventSequence
		)
			throw priorityProtocolError(
				"repair cursor moved backwards",
				snapshot.cursor,
			);
		if (
			this.state.authorityRevision !== null &&
			projection.revision < this.state.authorityRevision
		)
			throw priorityProtocolError(
				"repair revision moved backwards",
				snapshot.cursor,
			);
	}

	private requireEventSequence(sequence: number) {
		if (this.state.eventSequence === null)
			throw priorityProtocolError(
				"event arrived before its snapshot",
				sequence,
			);
		return this.state.eventSequence;
	}

	private acceptDuplicate(change: ProgrammerPriorityChange, sequence: number) {
		assertDuplicatePriorityChange(
			change,
			this.authoritative,
			this.state.authorityRevision,
			sequence,
		);
		return true;
	}

	private assertNextChange(change: ProgrammerPriorityChange, sequence: number) {
		assertNextPriorityChange(
			change,
			this.authoritative,
			this.state.authorityRevision,
			sequence,
		);
	}

	private publishAuthority(
		projection: ProgrammerPriorityProjection,
		sequence: number,
	) {
		this.authoritative = projection;
		this.state = {
			...this.state,
			eventSequence: sequence,
			authorityRevision: projection.revision,
			status: "ready",
			error: null,
			repairRequired: false,
		};
		this.publishRendered();
	}

	private publishTombstone(
		change: Extract<ProgrammerPriorityChange, { type: "remove" }>,
		sequence: number,
	) {
		this.authoritative = null;
		this.operations.clear();
		this.state = {
			...this.state,
			eventSequence: sequence,
			authorityRevision: change.revision,
			projection: null,
			status: "ready",
			error: null,
			repairRequired: false,
			pendingRequestIds: [],
		};
		this.emit();
	}

	private publishRendered(update: Partial<ProgrammerPriorityState> = {}) {
		this.state = {
			...this.state,
			...update,
			projection: renderOptimisticPriority(
				this.authoritative,
				this.operations.values(),
			),
			pendingRequestIds: [...this.operations.keys()],
		};
		this.emit();
	}

	private publishState(
		update: Partial<ProgrammerPriorityState>,
		expectedScope: number,
	) {
		if (!this.isScopeCurrent(expectedScope)) return false;
		this.state = { ...this.state, ...update };
		this.emit();
		return true;
	}

	private canAccept(userId: string, expectedScope: number) {
		return this.isScopeCurrent(expectedScope) && this.matchesUser(userId);
	}

	private matchesUser(userId: string) {
		return Boolean(
			this.state.userId &&
				userId.toLowerCase() === this.state.userId.toLowerCase(),
		);
	}

	private rejectProtocol(reason: unknown, sequence: number | null): never {
		const error =
			reason instanceof ProgrammerPriorityProtocolError
				? reason
				: new ProgrammerPriorityProtocolError(String(reason), sequence);
		this.markProtocolRepair(error, sequence);
		throw error;
	}

	private markProtocolRepair(reason: unknown, sequence: number | null) {
		const error =
			reason instanceof ProgrammerPriorityProtocolError
				? reason
				: new ProgrammerPriorityProtocolError(String(reason), sequence);
		this.state = {
			...this.state,
			status: "error",
			error,
			repairRequired: true,
		};
		this.emit();
		return error;
	}

	private emit() {
		for (const listener of this.listeners) listener();
	}
}
