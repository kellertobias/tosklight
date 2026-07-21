import type {
	ProgrammerPreloadPlaybackQueueProjection,
	ProgrammerPreloadPlaybackQueueSnapshot,
} from "./contracts";
import {
	assertPreloadPlaybackQueueCursor,
	canonicalPreloadPlaybackQueueProjection,
} from "./projectionValue";
import { ProgrammerPreloadPlaybackQueueProtocolError } from "./transport";

export type ProgrammerPreloadPlaybackQueueStatus =
	| "idle"
	| "loading"
	| "ready"
	| "error";

export interface ProgrammerPreloadPlaybackQueueState {
	showId: string | null;
	userId: string | null;
	authorityKey: string | null;
	eventSequence: number | null;
	projection: ProgrammerPreloadPlaybackQueueProjection | null;
	status: ProgrammerPreloadPlaybackQueueStatus;
	error: Error | null;
	repairRequired: boolean;
}

export class ProgrammerPreloadPlaybackQueueStore {
	private readonly listeners = new Set<() => void>();
	private scope = 0;
	private state = emptyState();

	readonly subscribe = (listener: () => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};

	readonly getSnapshot = () => this.state;

	reset(
		showId: string | null,
		userId: string | null,
		authorityKey: string | null,
	) {
		if (this.matchesScope(showId, userId, authorityKey)) return;
		this.scope++;
		this.state = { ...emptyState(), showId, userId, authorityKey };
		this.emit();
	}

	installSnapshot(
		snapshot: ProgrammerPreloadPlaybackQueueSnapshot,
		expectedScope = this.scope,
	) {
		return this.installSnapshotValue(snapshot, expectedScope, false);
	}

	installRepairSnapshot(
		snapshot: ProgrammerPreloadPlaybackQueueSnapshot,
		expectedScope = this.scope,
	) {
		return this.installSnapshotValue(snapshot, expectedScope, true);
	}

	applyProjection(
		projection: ProgrammerPreloadPlaybackQueueProjection,
		sequence: number,
		expectedScope = this.scope,
	) {
		if (!this.isScopeCurrent(expectedScope)) return false;
		try {
			assertPreloadPlaybackQueueCursor(sequence);
			const current = this.requireProjection(sequence);
			const currentSequence = this.requireSequence(sequence);
			if (sequence <= currentSequence) return true;
			const incoming = this.canonicalForCurrentUser(projection);
			this.assertNextRevision(incoming.revision, current.revision, sequence);
			this.publishProjection(incoming, sequence);
			return true;
		} catch (reason) {
			throw this.markProtocolRepair(reason, sequence);
		}
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
			? (this.state.projection?.revision ?? null)
			: null;
	}

	private matchesScope(
		showId: string | null,
		userId: string | null,
		authorityKey: string | null,
	) {
		return (
			showId === this.state.showId &&
			userId === this.state.userId &&
			authorityKey === this.state.authorityKey
		);
	}

	private installSnapshotValue(
		snapshot: ProgrammerPreloadPlaybackQueueSnapshot,
		expectedScope: number,
		repair: boolean,
	) {
		if (!this.isScopeCurrent(expectedScope)) return false;
		try {
			assertPreloadPlaybackQueueCursor(snapshot.cursor);
			const projection = this.canonicalForCurrentUser(snapshot.projection);
			if (repair) this.assertRepairDoesNotRegress(snapshot, projection);
			this.publishProjection(projection, snapshot.cursor);
			return true;
		} catch (reason) {
			throw this.markProtocolRepair(reason, snapshot.cursor);
		}
	}

	private canonicalForCurrentUser(
		projection: ProgrammerPreloadPlaybackQueueProjection,
	) {
		if (projection.userId !== this.state.userId)
			throw protocolError("authority does not match the active user", null);
		return canonicalPreloadPlaybackQueueProjection(projection);
	}

	private assertRepairDoesNotRegress(
		snapshot: ProgrammerPreloadPlaybackQueueSnapshot,
		projection: ProgrammerPreloadPlaybackQueueProjection,
	) {
		if (
			this.state.eventSequence !== null &&
			snapshot.cursor < this.state.eventSequence
		)
			throw protocolError("repair cursor moved backwards", snapshot.cursor);
		if (
			this.state.projection &&
			projection.revision < this.state.projection.revision
		)
			throw protocolError("repair revision moved backwards", snapshot.cursor);
	}

	private requireProjection(sequence: number) {
		if (!this.state.projection)
			throw protocolError("event arrived before its snapshot", sequence);
		return this.state.projection;
	}

	private requireSequence(sequence: number) {
		if (this.state.eventSequence === null)
			throw protocolError("event arrived without a cursor", sequence);
		return this.state.eventSequence;
	}

	private assertNextRevision(
		revision: number,
		current: number,
		sequence: number,
	) {
		if (revision !== current + 1)
			throw protocolError("projection revision is not contiguous", sequence);
	}

	private publishProjection(
		projection: ProgrammerPreloadPlaybackQueueProjection,
		eventSequence: number,
	) {
		this.state = {
			...this.state,
			projection,
			eventSequence,
			status: "ready",
			error: null,
			repairRequired: false,
		};
		this.emit();
	}

	private publishSessionState(
		update: Partial<ProgrammerPreloadPlaybackQueueState>,
		expectedScope: number,
	) {
		if (!this.isScopeCurrent(expectedScope)) return false;
		this.state = { ...this.state, ...update };
		this.emit();
		return true;
	}

	private markProtocolRepair(reason: unknown, sequence: number | null) {
		const error =
			reason instanceof ProgrammerPreloadPlaybackQueueProtocolError
				? reason
				: new ProgrammerPreloadPlaybackQueueProtocolError(
						String(reason),
						sequence,
					);
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

function emptyState(): ProgrammerPreloadPlaybackQueueState {
	return {
		showId: null,
		userId: null,
		authorityKey: null,
		eventSequence: null,
		projection: null,
		status: "idle",
		error: null,
		repairRequired: false,
	};
}

function protocolError(message: string, sequence: number | null) {
	return new ProgrammerPreloadPlaybackQueueProtocolError(
		`Preload playback queue ${message}`,
		sequence,
	);
}
