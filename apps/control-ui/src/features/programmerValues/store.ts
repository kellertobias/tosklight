import type {
	ProgrammerValuesProjection,
	ProgrammerValuesSnapshot,
} from "./contracts";
import {
	chooseProgrammerValuesAuthority,
	chooseProgrammerValuesRevision,
} from "./authority";
import { assertCursor, canonicalProjection } from "./projectionValue";
import {
	emptyProgrammerValuesState,
	type ProgrammerValuesOptimisticReducer,
	type ProgrammerValuesSettlement,
	type ProgrammerValuesState,
} from "./storeState";
import { ProgrammerValuesProtocolError } from "./transport";

export type {
	ProgrammerValuesOptimisticReducer,
	ProgrammerValuesSettlement,
	ProgrammerValuesState,
	ProgrammerValuesStatus,
} from "./storeState";

interface OptimisticOperation {
	requestId: string;
	apply: ProgrammerValuesOptimisticReducer;
}

export class ProgrammerValuesStore {
	private readonly listeners = new Set<() => void>();
	private readonly operations = new Map<string, OptimisticOperation>();
	private authoritative: ProgrammerValuesProjection | null = null;
	private authorityKey: string | null = null;
	private scope = 0;
	private state: ProgrammerValuesState = emptyProgrammerValuesState();

	readonly subscribe = (listener: () => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};

	readonly getSnapshot = () => this.state;

	reset(showId: string | null, userId: string | null, authorityKey = "") {
		if (
			showId === this.state.showId &&
			userId === this.state.userId &&
			authorityKey === this.authorityKey
		)
			return;
		this.scope++;
		this.authoritative = null;
		this.authorityKey = authorityKey;
		this.operations.clear();
		this.state = { ...emptyProgrammerValuesState(), showId, userId };
		this.emit();
	}

	installSnapshot(
		snapshot: ProgrammerValuesSnapshot,
		options: { expectedScope?: number; updateStatus?: boolean } = {},
	) {
		return this.install(
			snapshot.projection,
			snapshot.cursor,
			options.expectedScope ?? this.scope,
			options.updateStatus ?? true,
		);
	}

	installRepairSnapshot(
		snapshot: ProgrammerValuesSnapshot,
		expectedScope = this.scope,
	) {
		if (!this.canAccept(snapshot.projection.userId, expectedScope)) return false;
		try {
			assertCursor(snapshot.cursor);
			if (
				this.state.eventSequence !== null &&
				snapshot.cursor < this.state.eventSequence
			)
				throw new ProgrammerValuesProtocolError(
					"Programmer values repair snapshot moved its cursor backwards",
					snapshot.cursor,
				);
			const authoritative = canonicalProjection(snapshot.projection);
			this.publishAuthority(authoritative, snapshot.cursor, {
				status: "ready",
				error: null,
				repairRequired: false,
			});
			return true;
		} catch (reason) {
			return this.rejectProtocol(reason, snapshot.cursor);
		}
	}

	applyProjection(
		projection: ProgrammerValuesProjection,
		sequence: number,
		expectedScope = this.scope,
	) {
		return this.install(projection, sequence, expectedScope, true);
	}

	beginOptimistic(
		requestId: string,
		reducer: ProgrammerValuesOptimisticReducer,
		expectedScope = this.scope,
	) {
		if (!requestId || !this.isScopeCurrent(expectedScope) || !this.authoritative)
			return false;
		if (this.operations.has(requestId))
			throw new Error(`Programmer values request ${requestId} is already pending`);
		const apply = this.scopedReducer(reducer);
		const current = this.renderProjection();
		const rendered = apply(current);
		this.operations.set(requestId, { requestId, apply });
		this.publishRendered(rendered);
		return true;
	}

	settleChanged(
		requestId: string,
		projection: ProgrammerValuesProjection,
		sequence: number,
		expectedScope = this.scope,
	): ProgrammerValuesSettlement {
		if (!this.hasOperation(requestId, expectedScope)) return "ignored";
		try {
			if (!this.matchesUser(projection.userId)) return "ignored";
			assertCursor(sequence);
			const incoming = canonicalProjection(projection);
			const decision = this.chooseAuthority(incoming, sequence);
			this.settleOperation(requestId, decision.projection, {
				eventSequence: decision.sequence,
				error: null,
			});
			return "settled";
		} catch (reason) {
			this.markProtocolRepair(reason, sequence);
			return "repair";
		}
	}

	settleNoChange(
		requestId: string,
		revision: number,
		expectedScope = this.scope,
	): ProgrammerValuesSettlement {
		if (!this.hasOperation(requestId, expectedScope) || !this.authoritative)
			return "ignored";
		if (!Number.isSafeInteger(revision) || revision < 0) {
			this.markProtocolRepair(
				new Error("Programmer values outcome has an invalid revision"),
				this.state.eventSequence,
			);
			return "repair";
		}
		if (revision > this.authoritative.revision) return "repair";
		this.settleOperation(requestId, this.authoritative, { error: null });
		return "settled";
	}

	commit(
		requestId: string,
		projection?: ProgrammerValuesProjection,
		expectedScope = this.scope,
	) {
		if (!this.hasOperation(requestId, expectedScope)) return false;
		let nextAuthority = this.authoritative;
		try {
			if (projection) {
				if (!this.matchesUser(projection.userId)) return false;
				const incoming = canonicalProjection(projection);
				nextAuthority = chooseProgrammerValuesRevision(
					this.authoritative,
					incoming,
					this.state.eventSequence,
				).projection;
			}
			if (!nextAuthority) return false;
			const rendered = this.renderWithout(requestId, nextAuthority);
			this.operations.delete(requestId);
			this.authoritative = nextAuthority;
			this.publishRendered(rendered, { error: null });
			return true;
		} catch (reason) {
			return this.rejectProtocol(reason, this.state.eventSequence);
		}
	}

	rollback(
		requestId: string,
		error: Error,
		expectedScope = this.scope,
	) {
		if (!this.hasOperation(requestId, expectedScope) || !this.authoritative)
			return false;
		const rendered = this.renderWithout(requestId, this.authoritative);
		this.operations.delete(requestId);
		this.publishRendered(rendered, { error });
		return true;
	}

	setLoading(expectedScope = this.scope) {
		return this.publishSessionState(
			{ status: "loading", error: null },
			expectedScope,
		);
	}

	setReady(expectedScope = this.scope) {
		return this.publishSessionState(
			{ status: "ready", error: null, repairRequired: false },
			expectedScope,
		);
	}

	setError(error: Error, expectedScope = this.scope) {
		return this.publishSessionState({ status: "error", error }, expectedScope);
	}

	setRepairRequired(error: Error, expectedScope = this.scope) {
		return this.publishSessionState(
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
			? (this.authoritative?.revision ?? null)
			: null;
	}

	private install(
		projection: ProgrammerValuesProjection,
		sequence: number,
		expectedScope: number,
		updateStatus: boolean,
	) {
		if (!this.canAccept(projection.userId, expectedScope)) return false;
		try {
			assertCursor(sequence);
			const incoming = canonicalProjection(projection);
			const decision = this.chooseAuthority(incoming, sequence);
			if (!decision.publish) return true;
			const update = updateStatus
				? { status: "ready" as const, error: null, repairRequired: false }
				: {};
			this.publishAuthority(decision.projection, decision.sequence, update);
			return true;
		} catch (reason) {
			return this.rejectProtocol(reason, sequence);
		}
	}

	private chooseAuthority(
		incoming: ProgrammerValuesProjection,
		sequence: number,
	) {
		return chooseProgrammerValuesAuthority(
			this.authoritative,
			this.state.eventSequence,
			incoming,
			sequence,
		);
	}

	private publishAuthority(
		authoritative: ProgrammerValuesProjection,
		sequence: number | null,
		update: Partial<ProgrammerValuesState>,
	) {
		const rendered = this.renderFrom(authoritative);
		this.authoritative = authoritative;
		this.publishRendered(rendered, { ...update, eventSequence: sequence });
	}

	private scopedReducer(reducer: ProgrammerValuesOptimisticReducer) {
		return (current: ProgrammerValuesProjection) => {
			const reduced = reducer(current);
			if (Object.is(reduced, current)) return current;
			return canonicalProjection({
				...reduced,
				userId: current.userId,
				revision: current.revision,
			});
		};
	}

	private settleOperation(
		requestId: string,
		authoritative: ProgrammerValuesProjection,
		update: Partial<ProgrammerValuesState>,
	) {
		const rendered = this.renderWithout(requestId, authoritative);
		this.operations.delete(requestId);
		this.authoritative = authoritative;
		this.publishRendered(rendered, update);
	}

	private renderProjection() {
		if (!this.authoritative)
			throw new Error("Programmer values authority is not available");
		return this.renderFrom(this.authoritative);
	}

	private renderFrom(authoritative: ProgrammerValuesProjection) {
		let projection = authoritative;
		for (const operation of this.operations.values())
			projection = operation.apply(projection);
		return projection;
	}

	private renderWithout(
		requestId: string,
		authoritative: ProgrammerValuesProjection,
	) {
		let projection = authoritative;
		for (const operation of this.operations.values())
			if (operation.requestId !== requestId)
				projection = operation.apply(projection);
		return projection;
	}

	private hasOperation(requestId: string, expectedScope: number) {
		return (
			this.isScopeCurrent(expectedScope) && this.operations.has(requestId)
		);
	}

	private canAccept(userId: string, expectedScope: number) {
		return this.isScopeCurrent(expectedScope) && this.matchesUser(userId);
	}

	private matchesUser(userId: string) {
		return Boolean(this.state.showId) && userId === this.state.userId;
	}

	private publishSessionState(
		update: Partial<ProgrammerValuesState>,
		expectedScope: number,
	) {
		if (!this.isScopeCurrent(expectedScope)) return false;
		this.state = { ...this.state, ...update };
		this.emit();
		return true;
	}

	private publishRendered(
		projection: ProgrammerValuesProjection,
		update: Partial<ProgrammerValuesState> = {},
	) {
		this.state = {
			...this.state,
			projection,
			pendingRequestIds: Object.freeze([...this.operations.keys()]),
			...update,
		};
		this.emit();
	}

	private rejectProtocol(reason: unknown, sequence: number | null): never {
		throw this.markProtocolRepair(reason, sequence);
	}

	private markProtocolRepair(reason: unknown, sequence: number | null) {
		const error =
			reason instanceof ProgrammerValuesProtocolError
				? reason
				: new ProgrammerValuesProtocolError(String(reason), sequence);
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
