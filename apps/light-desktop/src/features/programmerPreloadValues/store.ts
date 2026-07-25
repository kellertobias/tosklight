import { choosePreloadAuthority, choosePreloadRevision } from "./authority";
import type {
	ProgrammerPreloadValuesProjection,
	ProgrammerPreloadValuesSnapshot,
} from "./contracts";
import {
	assertPreloadCursor,
	canonicalPreloadProjection,
} from "./projectionValue";
import {
	emptyProgrammerPreloadValuesState,
	type ProgrammerPreloadValuesOptimisticReducer,
	type ProgrammerPreloadValuesSettlement,
	type ProgrammerPreloadValuesState,
} from "./storeState";
import { ProgrammerPreloadValuesProtocolError } from "./transport";

export type {
	ProgrammerPreloadValuesOptimisticReducer,
	ProgrammerPreloadValuesSettlement,
	ProgrammerPreloadValuesState,
	ProgrammerPreloadValuesStatus,
} from "./storeState";

interface OptimisticOperation {
	requestId: string;
	apply: ProgrammerPreloadValuesOptimisticReducer;
}

export class ProgrammerPreloadValuesStore {
	private readonly listeners = new Set<() => void>();
	private readonly operations = new Map<string, OptimisticOperation>();
	private authoritative: ProgrammerPreloadValuesProjection | null = null;
	private authorityKey: string | null = null;
	private scope = 0;
	private state = emptyProgrammerPreloadValuesState();

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
		this.state = { ...emptyProgrammerPreloadValuesState(), showId, userId };
		this.emit();
	}

	installSnapshot(
		snapshot: ProgrammerPreloadValuesSnapshot,
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
		snapshot: ProgrammerPreloadValuesSnapshot,
		expectedScope = this.scope,
	) {
		if (!this.canAccept(snapshot.projection.userId, expectedScope))
			return false;
		try {
			assertPreloadCursor(snapshot.cursor);
			if (
				this.state.eventSequence !== null &&
				snapshot.cursor < this.state.eventSequence
			)
				throw new ProgrammerPreloadValuesProtocolError(
					"Preload Programmer values repair snapshot moved its cursor backwards",
					snapshot.cursor,
				);
			const authoritative = canonicalPreloadProjection(snapshot.projection);
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
		projection: ProgrammerPreloadValuesProjection,
		sequence: number,
		expectedScope = this.scope,
	) {
		return this.install(projection, sequence, expectedScope, true);
	}

	beginOptimistic(
		requestId: string,
		reducer: ProgrammerPreloadValuesOptimisticReducer,
		expectedScope = this.scope,
	) {
		if (
			!requestId ||
			!this.isScopeCurrent(expectedScope) ||
			!this.authoritative
		)
			return false;
		if (this.operations.has(requestId))
			throw new Error(
				`Preload Programmer values request ${requestId} is already pending`,
			);
		const apply = this.scopedReducer(reducer);
		const current = this.renderProjection();
		const rendered = apply(current);
		this.operations.set(requestId, { requestId, apply });
		this.publishRendered(rendered);
		return true;
	}

	settleChanged(
		requestId: string,
		projection: ProgrammerPreloadValuesProjection,
		sequence: number,
		expectedScope = this.scope,
	): ProgrammerPreloadValuesSettlement {
		if (!this.hasOperation(requestId, expectedScope)) return "ignored";
		try {
			if (!this.matchesUser(projection.userId)) return "ignored";
			assertPreloadCursor(sequence);
			const incoming = canonicalPreloadProjection(projection);
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
	): ProgrammerPreloadValuesSettlement {
		if (!this.hasOperation(requestId, expectedScope) || !this.authoritative)
			return "ignored";
		if (!Number.isSafeInteger(revision) || revision < 0) {
			this.markProtocolRepair(
				new Error("Preload Programmer values outcome has an invalid revision"),
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
		projection?: ProgrammerPreloadValuesProjection,
		expectedScope = this.scope,
	) {
		if (!this.hasOperation(requestId, expectedScope)) return false;
		let nextAuthority = this.authoritative;
		try {
			if (projection) {
				if (!this.matchesUser(projection.userId)) return false;
				const incoming = canonicalPreloadProjection(projection);
				nextAuthority = choosePreloadRevision(
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

	rollback(requestId: string, error: Error, expectedScope = this.scope) {
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
		projection: ProgrammerPreloadValuesProjection,
		sequence: number,
		expectedScope: number,
		updateStatus: boolean,
	) {
		if (!this.canAccept(projection.userId, expectedScope)) return false;
		try {
			assertPreloadCursor(sequence);
			const incoming = canonicalPreloadProjection(projection);
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
		incoming: ProgrammerPreloadValuesProjection,
		sequence: number,
	) {
		return choosePreloadAuthority(
			this.authoritative,
			this.state.eventSequence,
			incoming,
			sequence,
		);
	}

	private publishAuthority(
		authoritative: ProgrammerPreloadValuesProjection,
		sequence: number | null,
		update: Partial<ProgrammerPreloadValuesState>,
	) {
		const rendered = this.renderFrom(authoritative);
		this.authoritative = authoritative;
		this.publishRendered(rendered, { ...update, eventSequence: sequence });
	}

	private scopedReducer(reducer: ProgrammerPreloadValuesOptimisticReducer) {
		return (current: ProgrammerPreloadValuesProjection) => {
			const reduced = reducer(current);
			if (Object.is(reduced, current)) return current;
			return canonicalPreloadProjection({
				...reduced,
				userId: current.userId,
				revision: current.revision,
			});
		};
	}

	private settleOperation(
		requestId: string,
		authoritative: ProgrammerPreloadValuesProjection,
		update: Partial<ProgrammerPreloadValuesState>,
	) {
		const rendered = this.renderWithout(requestId, authoritative);
		this.operations.delete(requestId);
		this.authoritative = authoritative;
		this.publishRendered(rendered, update);
	}

	private renderProjection() {
		if (!this.authoritative)
			throw new Error("Preload Programmer values authority is not available");
		return this.renderFrom(this.authoritative);
	}

	private renderFrom(authoritative: ProgrammerPreloadValuesProjection) {
		let projection = authoritative;
		for (const operation of this.operations.values())
			projection = operation.apply(projection);
		return projection;
	}

	private renderWithout(
		requestId: string,
		authoritative: ProgrammerPreloadValuesProjection,
	) {
		let projection = authoritative;
		for (const operation of this.operations.values())
			if (operation.requestId !== requestId)
				projection = operation.apply(projection);
		return projection;
	}

	private hasOperation(requestId: string, expectedScope: number) {
		return this.isScopeCurrent(expectedScope) && this.operations.has(requestId);
	}

	private canAccept(userId: string, expectedScope: number) {
		return this.isScopeCurrent(expectedScope) && this.matchesUser(userId);
	}

	private matchesUser(userId: string) {
		return Boolean(this.state.showId) && userId === this.state.userId;
	}

	private publishSessionState(
		update: Partial<ProgrammerPreloadValuesState>,
		expectedScope: number,
	) {
		if (!this.isScopeCurrent(expectedScope)) return false;
		this.state = { ...this.state, ...update };
		this.emit();
		return true;
	}

	private publishRendered(
		projection: ProgrammerPreloadValuesProjection,
		update: Partial<ProgrammerPreloadValuesState> = {},
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
			reason instanceof ProgrammerPreloadValuesProtocolError
				? reason
				: new ProgrammerPreloadValuesProtocolError(String(reason), sequence);
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
