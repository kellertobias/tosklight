import type {
	PlaybackDesk,
	PlaybackIdentity,
	PlaybackOutcome,
	PlaybackProjection,
	PlaybackSnapshot,
} from "./contracts";
import { identityKey, projectionKeys } from "./contracts";
import { PlaybackDeskState } from "./deskState";
import { optimisticMaster } from "./optimistic";
import { projectSnapshot, upsertProjection } from "./projectionCollection";
import { PlaybackRequestTracker } from "./requestTracker";
import {
	outcomeEventSequence,
	outcomeMatchesScope,
	outcomeShowRevision,
} from "./outcome";

export interface PlaybackRuntimeState {
	showId: string | null;
	deskId: string | null;
	showRevision: number | null;
	eventSequence: number | null;
	desk: PlaybackDesk | null;
	projections: ReadonlyMap<string, readonly PlaybackProjection[]>;
	pendingKeys: ReadonlySet<string>;
	status: "idle" | "loading" | "ready" | "error";
	error: Error | null;
}

export class PlaybackRuntimeStore {
	private readonly listeners = new Set<() => void>();
	private readonly keySequences = new Map<string, number>();
	private readonly authoritativeProjections = new Map<
		string,
		readonly PlaybackProjection[]
	>();
	private readonly deskState = new PlaybackDeskState();
	private readonly requests = new PlaybackRequestTracker();
	private authorityKey = "";
	private scope = 0;
	private state: PlaybackRuntimeState = emptyState();

	readonly subscribe = (listener: () => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};

	readonly getSnapshot = () => this.state;

	reset(showId: string | null, deskId: string | null, authorityKey = "") {
		if (
			showId === this.state.showId &&
			deskId === this.state.deskId &&
			authorityKey === this.authorityKey
		)
			return;
		this.scope++;
		this.authorityKey = authorityKey;
		this.keySequences.clear();
		this.authoritativeProjections.clear();
		this.requests.reset();
		this.deskState.reset();
		this.state = {
			...emptyState(),
			showId,
			deskId,
			status: showId && deskId ? "loading" : "idle",
		};
		this.emit();
	}

	captureScope() {
		return this.scope;
	}

	isScopeCurrent(scope: number) {
		return scope === this.scope;
	}

	seedDesk(activePage: number, selectedPlayback: number | null, revision = 0) {
		const { showId, deskId } = this.state;
		if (!showId || !deskId || this.state.desk) return;
		const desk = this.deskState.seed({
			scope: { show_id: showId, show_revision: revision },
			desk_id: deskId,
			active_page: activePage,
			selected_playback: selectedPlayback,
		});
		this.publish({ desk, showRevision: revision });
	}

	setLoading() {
		if (this.state.status !== "loading") this.publish({ status: "loading" });
	}

	setReady() {
		this.publish({ status: "ready", error: null });
	}

	setError(error: Error) {
		this.publish({ status: "error", error });
	}

	installSnapshot(
		snapshot: PlaybackSnapshot,
		identities: readonly PlaybackIdentity[],
	) {
		if (!this.matches(snapshot.desk.scope.show_id, snapshot.desk.desk_id))
			return;
		const sequence = snapshot.cursor.sequence;
		const incoming = projectSnapshot(snapshot.projections, identities);
		const requested = new Set(identities.map(identityKey));
		const projections = new Map(this.state.projections);
		for (const [key, values] of incoming) {
			if ((this.keySequences.get(key) ?? 0) > sequence) continue;
			const authoritative = requested.has(key)
				? values
				: values.reduce(
						(current, projection) => upsertProjection(current, projection),
						this.authoritativeProjections.get(key) ?? [],
					);
			this.authoritativeProjections.set(key, authoritative);
			this.keySequences.set(key, sequence);
			projections.set(key, this.renderProjectionKey(key));
		}
		const desk = this.deskState.hydrate(snapshot.desk, sequence);
		this.publish({
			projections,
			desk: desk ?? this.state.desk,
			showRevision: snapshot.desk.scope.show_revision,
			eventSequence: Math.max(this.state.eventSequence ?? 0, sequence),
			status: "ready",
			error: null,
		});
	}

	applyProjection(projection: PlaybackProjection, sequence: number) {
		const projections = new Map(this.state.projections);
		if (!this.mergeProjection(projection, sequence, undefined, projections))
			return false;
		this.publish({
			projections,
			showRevision: Math.max(
				this.state.showRevision ?? 0,
				projection.scope.show_revision,
			),
			eventSequence: Math.max(this.state.eventSequence ?? 0, sequence),
			status: "ready",
			error: null,
		});
		return true;
	}

	applyDesk(projection: PlaybackDesk, sequence: number) {
		if (!this.matches(projection.scope.show_id, projection.desk_id))
			return false;
		const desk = this.deskState.apply(projection, sequence);
		if (!desk) return false;
		this.publish({
			desk,
			showRevision: Math.max(
				this.state.showRevision ?? 0,
				projection.scope.show_revision,
			),
			eventSequence: Math.max(this.state.eventSequence ?? 0, sequence),
			status: "ready",
			error: null,
		});
		return true;
	}

	installOutcome(outcome: PlaybackOutcome, token?: string | null) {
		const pending = this.requests.peek(token);
		if (token && !pending) return false;
		if (pending) this.assertOutcomeMatchesRequest(outcome, pending.key);
		const operation = this.requests.take(token);
		if (
			!operation &&
			!outcomeMatchesScope(outcome, this.state.showId, this.state.deskId)
		)
			return false;
		const projections = new Map(this.state.projections);
		const matchesOutcome = this.matches(outcome.projection.scope.show_id);
		for (const related of outcome.related)
			this.mergeProjection(
				related.projection,
				related.event_sequence,
				operation?.baseSequences,
				projections,
			);
		const sequence = outcome.event_sequence ?? null;
		this.mergeProjection(
			outcome.projection,
			sequence,
			operation?.baseSequences,
			projections,
		);
		if (operation)
			projections.set(operation.key, this.renderProjectionKey(operation.key));
		this.publish({
			projections,
			desk: this.outcomeDesk(outcome, sequence, operation?.deskBaseSequence),
			showRevision: outcomeShowRevision(
				outcome,
				this.state.showId,
				this.state.showRevision,
			),
			eventSequence: outcomeEventSequence(
				outcome,
				this.state.showId,
				this.state.deskId,
				this.state.eventSequence,
			),
			pendingKeys: this.pendingKeys(),
			status: matchesOutcome ? "ready" : this.state.status,
			error: null,
		});
		return true;
	}

	beginOptimisticMaster(playbackNumber: number, value: number) {
		const key = `playback:${playbackNumber}`;
		const current = this.state.projections.get(key) ?? [];
		const projection = current.find(
			(candidate) => candidate.playback_number === playbackNumber,
		);
		if (!projection) return null;
		const optimistic = optimisticMaster(projection, value);
		if (!optimistic) return null;
		const token = this.beginTrackedRequest(key, optimistic);
		const projections = new Map(this.state.projections);
		projections.set(key, this.renderProjectionKey(key));
		this.publish({ projections, pendingKeys: this.pendingKeys(), error: null });
		return token;
	}

	beginRequest(identity: PlaybackIdentity) {
		return this.beginTrackedRequest(identityKey(identity));
	}

	rollbackProjection(token: string | null, error: Error) {
		const operation = this.requests.take(token);
		if (!operation) return false;
		if (!operation.optimistic) return true;
		const projections = new Map(this.state.projections);
		projections.set(operation.key, this.renderProjectionKey(operation.key));
		this.publish({
			projections,
			pendingKeys: this.pendingKeys(),
			status: "error",
			error,
		});
		return true;
	}

	beginOptimisticPage(page: number) {
		const operation = this.deskState.begin(page);
		if (!operation) return null;
		this.publish({ desk: operation.desk, error: null });
		return operation.token;
	}

	commitPage(token: string | null, _page: number, sequence: number | null) {
		const desk = this.deskState.commit(token, sequence);
		if (!desk) return false;
		this.publish({ desk, error: null });
		return true;
	}

	rollbackPage(token: string | null, error: Error) {
		const desk = this.deskState.rollback(token);
		if (!desk) return false;
		this.publish({ desk, status: "error", error });
		return true;
	}

	private mergeProjection(
		projection: PlaybackProjection,
		sequence: number | null,
		baseSequences: ReadonlyMap<string, number> | undefined,
		projections: Map<string, readonly PlaybackProjection[]>,
	) {
		if (!this.matches(projection.scope.show_id)) return false;
		let changed = false;
		for (const key of projectionKeys(projection)) {
			const current = this.keySequences.get(key) ?? 0;
			const baseline = baseSequences?.get(key) ?? 0;
			if (sequence == null ? current > baseline : current >= sequence)
				continue;
			this.authoritativeProjections.set(
				key,
				upsertProjection(
					this.authoritativeProjections.get(key) ?? [],
					projection,
				),
			);
			if (sequence != null) this.keySequences.set(key, sequence);
			projections.set(key, this.renderProjectionKey(key));
			changed = true;
		}
		return changed;
	}

	private outcomeDesk(
		outcome: PlaybackOutcome,
		sequence: number | null,
		deskBaseSequence: number | undefined,
	) {
		if (
			!outcome.desk ||
			!this.matches(outcome.desk.scope.show_id, outcome.desk.desk_id)
		)
			return this.state.desk;
		const deskSequence = outcome.desk_event_sequence ?? sequence;
		if (deskSequence != null)
			return this.deskState.apply(outcome.desk, deskSequence) ?? this.state.desk;
		return deskBaseSequence == null
			? this.state.desk
			: this.deskState.installUnsequenced(outcome.desk, deskBaseSequence);
	}

	private beginTrackedRequest(
		key: string,
		optimistic: PlaybackProjection | null = null,
	) {
		const keys = new Set([key]);
		for (const projection of this.authoritativeProjections.get(key) ?? [])
			for (const projectionKey of projectionKeys(projection))
				keys.add(projectionKey);
		const baselines = new Map(
			[...keys].map((projectionKey) => [
				projectionKey,
				this.keySequences.get(projectionKey) ?? 0,
			]),
		);
		return this.requests.begin(
			key,
			baselines,
			this.deskState.captureSequence(),
			optimistic,
		);
	}

	private renderProjectionKey(key: string) {
		const authoritative = this.authoritativeProjections.get(key) ?? [];
		return this.requests.render(key, authoritative);
	}

	private pendingKeys() {
		return this.requests.pendingKeys();
	}

	private assertOutcomeMatchesRequest(
		outcome: PlaybackOutcome,
		requestKey: string,
	) {
		if (
			!this.matches(outcome.projection.scope.show_id) ||
			!projectionKeys(outcome.projection).includes(requestKey)
		)
			throw new Error("Playback outcome does not match the active request scope");
	}

	private matches(showId: string, deskId = this.state.deskId) {
		return showId === this.state.showId && deskId === this.state.deskId;
	}

	private publish(update: Partial<PlaybackRuntimeState>) {
		this.state = { ...this.state, ...update };
		this.emit();
	}

	private emit() {
		for (const listener of this.listeners) listener();
	}
}

function emptyState(): PlaybackRuntimeState {
	return {
		showId: null,
		deskId: null,
		showRevision: null,
		eventSequence: null,
		desk: null,
		projections: new Map(),
		pendingKeys: new Set(),
		status: "idle",
		error: null,
	};
}
