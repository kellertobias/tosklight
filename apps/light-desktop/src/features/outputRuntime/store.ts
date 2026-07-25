import type {
	OutputRuntimeChange,
	OutputRuntimeProjection,
	OutputRuntimeSnapshot,
} from "./contracts";
import {
	type OptimisticOutputMutation,
	renderOptimisticOutput,
} from "./optimisticProjection";
import {
	assertOutputCursor,
	assertOutputMutation,
	canonicalOutputProjection,
	sameOutputProjection,
} from "./projectionValue";
import {
	emptyOutputRuntimeState,
	type OutputRuntimeSettlement,
	type OutputRuntimeState,
} from "./storeState";
import { OutputRuntimeProtocolError } from "./transport";

export type {
	OutputRuntimeSettlement,
	OutputRuntimeState,
	OutputRuntimeStatus,
} from "./storeState";

export class OutputRuntimeStore {
	private readonly listeners = new Set<() => void>();
	private readonly operations = new Map<string, OptimisticOutputMutation>();
	private authoritative: OutputRuntimeProjection | null = null;
	private scope = 0;
	private state = emptyOutputRuntimeState();

	readonly subscribe = (listener: () => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};

	readonly getSnapshot = () => this.state;

	reset(showId: string | null, deskId: string | null, authorityKey = "") {
		if (
			showId === this.state.showId &&
			deskId === this.state.deskId &&
			authorityKey === this.state.authorityKey
		)
			return;
		this.scope++;
		this.authoritative = null;
		this.operations.clear();
		this.state = {
			...emptyOutputRuntimeState(),
			showId,
			deskId,
			authorityKey,
		};
		this.emit();
	}

	installSnapshot(snapshot: OutputRuntimeSnapshot, expectedScope = this.scope) {
		return this.installSnapshotValue(snapshot, expectedScope, false);
	}

	installRepairSnapshot(
		snapshot: OutputRuntimeSnapshot,
		expectedScope = this.scope,
	) {
		return this.installSnapshotValue(snapshot, expectedScope, true);
	}

	applyChange(
		change: OutputRuntimeChange,
		sequence: number,
		expectedScope = this.scope,
	) {
		if (!this.canAccept(change.projection.showId, expectedScope)) return false;
		try {
			assertOutputCursor(sequence);
			const currentSequence = this.requireEventSequence(sequence);
			if (sequence < currentSequence) return true;
			if (sequence === currentSequence)
				return this.acceptDuplicate(change, sequence);
			this.assertNextChange(change, sequence);
			this.publishAuthority(change.projection, sequence);
			return true;
		} catch (reason) {
			throw this.rejectProtocol(reason, sequence);
		}
	}

	beginOptimistic(
		operation: OptimisticOutputMutation,
		expectedScope = this.scope,
	) {
		if (
			!operation.requestId ||
			!this.isScopeCurrent(expectedScope) ||
			!this.authoritative ||
			this.state.status !== "ready" ||
			this.state.repairRequired
		)
			return false;
		if (this.operations.has(operation.requestId))
			throw new Error(
				`Output request ${operation.requestId} is already pending`,
			);
		assertOutputMutation(operation.grandMaster, operation.blackout);
		this.operations.set(operation.requestId, operation);
		this.publishRendered();
		return true;
	}

	settleChanged(
		requestId: string,
		projection: OutputRuntimeProjection,
		sequence: number,
		expectedScope = this.scope,
	): OutputRuntimeSettlement {
		if (!this.hasOperation(requestId, expectedScope)) return "ignored";
		try {
			if (!this.matchesShow(projection.showId)) return "ignored";
			this.applyChange({ projection }, sequence, expectedScope);
			this.operations.delete(requestId);
			this.publishRendered();
			return "settled";
		} catch {
			return "repair";
		}
	}

	settleNoChange(
		requestId: string,
		projection: OutputRuntimeProjection,
		expectedScope = this.scope,
	): OutputRuntimeSettlement {
		if (!this.hasOperation(requestId, expectedScope)) return "ignored";
		try {
			const incoming = canonicalOutputProjection(projection);
			if (!this.matchesShow(incoming.showId)) return "ignored";
			const revision = this.state.authorityRevision;
			if (revision === null || incoming.revision > revision) return "repair";
			if (
				incoming.revision === revision &&
				(!this.authoritative ||
					!sameOutputProjection(this.authoritative, incoming))
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
		snapshot: OutputRuntimeSnapshot,
		expectedScope: number,
		repair: boolean,
	) {
		if (!this.canAccept(snapshot.projection.showId, expectedScope))
			return false;
		try {
			assertOutputCursor(snapshot.cursor);
			const projection = canonicalOutputProjection(snapshot.projection);
			if (repair) this.assertRepairDoesNotRegress(snapshot, projection);
			this.publishCanonicalAuthority(projection, snapshot.cursor);
			return true;
		} catch (reason) {
			throw this.rejectProtocol(reason, snapshot.cursor);
		}
	}

	private assertRepairDoesNotRegress(
		snapshot: OutputRuntimeSnapshot,
		projection: OutputRuntimeProjection,
	) {
		if (
			this.state.eventSequence !== null &&
			snapshot.cursor < this.state.eventSequence
		)
			throw protocolError("repair cursor moved backwards", snapshot.cursor);
		if (
			this.state.authorityRevision !== null &&
			projection.revision < this.state.authorityRevision
		)
			throw protocolError("repair revision moved backwards", snapshot.cursor);
	}

	private requireEventSequence(sequence: number) {
		if (this.state.eventSequence === null)
			throw protocolError("event arrived before its snapshot", sequence);
		return this.state.eventSequence;
	}

	private acceptDuplicate(change: OutputRuntimeChange, sequence: number) {
		const incoming = canonicalOutputProjection(change.projection);
		if (
			!this.authoritative ||
			!sameOutputProjection(this.authoritative, incoming)
		)
			throw protocolError("event sequence conflicts", sequence);
		return true;
	}

	private assertNextChange(change: OutputRuntimeChange, sequence: number) {
		const revision = change.projection.revision;
		if (
			this.state.authorityRevision === null ||
			revision !== this.state.authorityRevision + 1
		)
			throw protocolError("event revision is not contiguous", sequence);
	}

	private publishAuthority(
		incoming: OutputRuntimeProjection,
		sequence: number,
	) {
		this.publishCanonicalAuthority(
			canonicalOutputProjection(incoming),
			sequence,
		);
	}

	private publishCanonicalAuthority(
		canonical: OutputRuntimeProjection,
		sequence: number,
	) {
		const projection =
			this.authoritative && sameOutputProjection(this.authoritative, canonical)
				? this.authoritative
				: canonical;
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

	private publishRendered(update: Partial<OutputRuntimeState> = {}) {
		this.state = {
			...this.state,
			...update,
			projection: renderOptimisticOutput(
				this.authoritative,
				this.operations.values(),
			),
			pendingRequestIds: [...this.operations.keys()],
		};
		this.emit();
	}

	private publishState(
		update: Partial<OutputRuntimeState>,
		expectedScope: number,
	) {
		if (!this.isScopeCurrent(expectedScope)) return false;
		this.state = { ...this.state, ...update };
		this.emit();
		return true;
	}

	private canAccept(showId: string, expectedScope: number) {
		return this.isScopeCurrent(expectedScope) && this.matchesShow(showId);
	}

	private matchesShow(showId: string) {
		return Boolean(
			this.state.showId &&
				showId.toLowerCase() === this.state.showId.toLowerCase(),
		);
	}

	private rejectProtocol(reason: unknown, sequence: number | null): never {
		throw this.markProtocolRepair(reason, sequence);
	}

	private markProtocolRepair(reason: unknown, sequence: number | null) {
		const error =
			reason instanceof OutputRuntimeProtocolError
				? reason
				: new OutputRuntimeProtocolError(String(reason), sequence);
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

function protocolError(message: string, sequence: number | null) {
	return new OutputRuntimeProtocolError(`Output runtime ${message}`, sequence);
}
