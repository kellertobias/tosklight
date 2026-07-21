import type {
	SpeedGroupActionOutcome,
	SpeedGroupAuthorityProjection,
	SpeedGroupChange,
	SpeedGroupSnapshot,
} from "./contracts";
import type { OptimisticSpeedGroupMutation } from "./optimisticProjection";
import { renderOptimisticSpeedGroups } from "./optimisticProjection";
import {
	assertNoChangeOutcome,
	authorityAfterChangedOutcome,
} from "./outcomeAuthority";
import {
	assertAction,
	assertCursor,
	assertRepairDoesNotRegress,
	canonicalAuthority,
	canonicalPartialGroups,
	mergeGroups,
	sameAuthority,
	sameAuthorityId,
	sameGroup,
	speedGroupProtocolError,
} from "./projectionValue";
import {
	emptySpeedGroupState,
	type SpeedGroupRuntimeState,
	type SpeedGroupSettlement,
} from "./storeState";
import { SpeedGroupProtocolError } from "./transport";

export class SpeedGroupRuntimeStore {
	private readonly listeners = new Set<() => void>();
	private readonly operations = new Map<string, OptimisticSpeedGroupMutation>();
	private authoritative: SpeedGroupAuthorityProjection | null = null;
	private scope = 0;
	private state = emptySpeedGroupState();

	readonly subscribe = (listener: () => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};
	readonly getSnapshot = () => this.state;

	reset(deskId: string | null, authorityKey = "") {
		if (
			deskId === this.state.deskId &&
			authorityKey === this.state.authorityKey
		)
			return;
		this.scope++;
		this.authoritative = null;
		this.operations.clear();
		this.state = { ...emptySpeedGroupState(), deskId, authorityKey };
		this.emit();
	}

	installSnapshot(snapshot: SpeedGroupSnapshot, expectedScope = this.scope) {
		return this.installSnapshotValue(snapshot, expectedScope, false);
	}

	installRepairSnapshot(
		snapshot: SpeedGroupSnapshot,
		expectedScope = this.scope,
	) {
		return this.installSnapshotValue(snapshot, expectedScope, true);
	}

	applyChange(
		change: SpeedGroupChange,
		sequence: number,
		expected = this.scope,
	) {
		if (!this.isScopeCurrent(expected)) return false;
		try {
			assertCursor(sequence);
			const currentSequence = this.requireEventSequence(sequence);
			if (sequence < currentSequence) return true;
			if (sequence === currentSequence)
				return this.acceptSameSequence(change, sequence);
			this.assertNextChange(change, sequence);
			this.publishChange(change, sequence);
			return true;
		} catch (reason) {
			throw this.rejectProtocol(reason, sequence);
		}
	}

	beginOptimistic(
		operation: OptimisticSpeedGroupMutation,
		expected = this.scope,
	) {
		if (
			!operation.requestId ||
			!this.isScopeCurrent(expected) ||
			!this.authoritative ||
			this.state.status !== "ready" ||
			this.state.repairRequired
		)
			return false;
		if (this.operations.has(operation.requestId))
			throw new Error(`Speed Group request ${operation.requestId} is pending`);
		assertAction(operation.action);
		canonicalAuthority(
			renderOptimisticSpeedGroups(this.authoritative, [
				...this.operations.values(),
				operation,
			]) ?? this.authoritative,
		);
		this.operations.set(operation.requestId, operation);
		this.publishRendered();
		return true;
	}

	settleChanged(
		requestId: string,
		outcome: SpeedGroupActionOutcome & { status: "changed" },
		expected = this.scope,
	): SpeedGroupSettlement {
		const operation = this.operation(requestId, expected);
		if (!operation) return "ignored";
		try {
			this.acceptChangedOutcome(operation, outcome);
			this.operations.delete(requestId);
			this.publishRendered();
			return "settled";
		} catch (reason) {
			this.markProtocolRepair(reason, outcome.eventSequence);
			return "repair";
		}
	}

	settleNoChange(
		requestId: string,
		outcome: SpeedGroupActionOutcome & { status: "no_change" },
		expected = this.scope,
	): SpeedGroupSettlement {
		if (!this.operation(requestId, expected)) return "ignored";
		try {
			if (
				!this.authoritative ||
				!assertNoChangeOutcome(this.authoritative, outcome)
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

	rollback(requestId: string, error: Error, expected = this.scope) {
		if (!this.operation(requestId, expected)) return false;
		this.operations.delete(requestId);
		this.publishRendered({ error });
		return true;
	}

	abandon(requestId: string, expected = this.scope) {
		if (!this.operation(requestId, expected)) return false;
		this.operations.delete(requestId);
		this.publishRendered();
		return true;
	}

	setLoading(expected = this.scope) {
		return this.publishState({ status: "loading", error: null }, expected);
	}
	setReady(expected = this.scope) {
		return this.publishState(
			{ status: "ready", error: null, repairRequired: false },
			expected,
		);
	}
	setError(error: Error, expected = this.scope) {
		return this.publishState({ status: "error", error }, expected);
	}
	setRepairRequired(error: Error, expected = this.scope) {
		return this.publishState(
			{ status: "error", error, repairRequired: true },
			expected,
		);
	}

	captureScope() {
		return this.scope;
	}
	isScopeCurrent(scope: number) {
		return scope === this.scope;
	}
	authority(expected = this.scope) {
		return this.isScopeCurrent(expected) && this.authoritative
			? {
					authorityId: this.authoritative.authorityId,
					revision: this.authoritative.revision,
				}
			: null;
	}
	hasOperation(requestId: string, expected = this.scope) {
		return Boolean(this.operation(requestId, expected));
	}

	private installSnapshotValue(
		snapshot: SpeedGroupSnapshot,
		expected: number,
		repair: boolean,
	) {
		if (!this.isScopeCurrent(expected)) return false;
		try {
			assertCursor(snapshot.cursor);
			const projection = canonicalAuthority(snapshot.projection);
			if (repair) assertRepairDoesNotRegress(this.state, snapshot);
			if (
				this.authoritative &&
				!sameAuthorityId(this.authoritative.authorityId, projection.authorityId)
			)
				this.operations.clear();
			this.publishAuthority(projection, snapshot.cursor);
			return true;
		} catch (reason) {
			throw this.rejectProtocol(reason, snapshot.cursor);
		}
	}

	private requireEventSequence(sequence: number) {
		if (this.state.eventSequence === null)
			throw speedGroupProtocolError(
				"event arrived before its snapshot",
				sequence,
			);
		return this.state.eventSequence;
	}

	private acceptSameSequence(change: SpeedGroupChange, sequence: number) {
		this.assertSameAuthorityRevision(change, sequence);
		if (!this.authoritative)
			throw speedGroupProtocolError("authority is absent", sequence);
		for (const group of canonicalPartialGroups(change.groups)) {
			const current = this.authoritative.groups.find(
				(candidate) => candidate.group === group.group,
			);
			if (!sameGroup(group, current))
				throw speedGroupProtocolError("event sequence conflicts", sequence);
		}
		return true;
	}

	private assertNextChange(change: SpeedGroupChange, sequence: number) {
		if (!this.authoritative)
			throw speedGroupProtocolError("authority is absent", sequence);
		if (!sameAuthorityId(change.authorityId, this.authoritative.authorityId))
			throw speedGroupProtocolError(
				"event authority changed without repair",
				sequence,
			);
		if (change.revision !== this.authoritative.revision + 1)
			throw speedGroupProtocolError(
				"event revision is not contiguous",
				sequence,
			);
	}

	private assertSameAuthorityRevision(
		change: SpeedGroupChange,
		sequence: number,
	) {
		if (
			!this.authoritative ||
			!sameAuthorityId(change.authorityId, this.authoritative.authorityId) ||
			change.revision !== this.authoritative.revision
		)
			throw speedGroupProtocolError("event sequence conflicts", sequence);
	}

	private publishChange(change: SpeedGroupChange, sequence: number) {
		if (!this.authoritative)
			throw speedGroupProtocolError("authority is absent", sequence);
		this.publishAuthority(
			mergeGroups(
				this.authoritative,
				change.authorityId,
				change.revision,
				change.groups,
			),
			sequence,
		);
	}

	private acceptChangedOutcome(
		operation: OptimisticSpeedGroupMutation,
		outcome: SpeedGroupActionOutcome & { status: "changed" },
	) {
		if (!this.authoritative)
			throw speedGroupProtocolError("authority is absent");
		const next = authorityAfterChangedOutcome(
			this.authoritative,
			operation,
			outcome,
		);
		if (next) this.publishAuthority(next, outcome.eventSequence);
	}

	private publishAuthority(
		incoming: SpeedGroupAuthorityProjection,
		sequence: number,
	) {
		const canonical = canonicalAuthority(incoming);
		const projection =
			this.authoritative && sameAuthority(this.authoritative, canonical)
				? this.authoritative
				: canonical;
		this.authoritative = projection;
		this.state = {
			...this.state,
			eventSequence: sequence,
			authorityId: projection.authorityId,
			authorityRevision: projection.revision,
			status: "ready",
			error: null,
			repairRequired: false,
		};
		this.publishRendered();
	}

	private operation(requestId: string, expected: number) {
		return this.isScopeCurrent(expected)
			? this.operations.get(requestId)
			: null;
	}

	private publishRendered(update: Partial<SpeedGroupRuntimeState> = {}) {
		const rendered = renderOptimisticSpeedGroups(
			this.authoritative,
			this.operations.values(),
		);
		this.state = {
			...this.state,
			...update,
			projection:
				rendered === this.authoritative
					? rendered
					: rendered
						? canonicalAuthority(rendered)
						: null,
			pendingRequestIds: [...this.operations.keys()],
		};
		this.emit();
	}

	private publishState(
		update: Partial<SpeedGroupRuntimeState>,
		expected: number,
	) {
		if (!this.isScopeCurrent(expected)) return false;
		this.state = { ...this.state, ...update };
		this.emit();
		return true;
	}

	private rejectProtocol(reason: unknown, sequence: number | null): never {
		throw this.markProtocolRepair(reason, sequence);
	}

	private markProtocolRepair(reason: unknown, sequence: number | null) {
		const error =
			reason instanceof SpeedGroupProtocolError
				? reason
				: new SpeedGroupProtocolError(String(reason), sequence);
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
