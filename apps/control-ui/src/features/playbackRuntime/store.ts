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

interface ProjectionOperation {
	token: string;
	key: string;
	baseSequence: number;
	order: number;
	optimistic: PlaybackProjection;
}

export class PlaybackRuntimeStore {
	private readonly listeners = new Set<() => void>();
	private readonly keySequences = new Map<string, number>();
	private readonly authoritativeProjections = new Map<
		string,
		readonly PlaybackProjection[]
	>();
	private readonly projectionOperations = new Map<
		string,
		ProjectionOperation
	>();
	private readonly latestProjectionTokens = new Map<string, string>();
	private readonly deskState = new PlaybackDeskState();
	private projectionOperationOrder = 0;
	private state: PlaybackRuntimeState = emptyState();

	readonly subscribe = (listener: () => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};

	readonly getSnapshot = () => this.state;

	reset(showId: string | null, deskId: string | null) {
		if (showId === this.state.showId && deskId === this.state.deskId) return;
		this.keySequences.clear();
		this.authoritativeProjections.clear();
		this.projectionOperations.clear();
		this.latestProjectionTokens.clear();
		this.deskState.reset();
		this.projectionOperationOrder = 0;
		this.state = {
			...emptyState(),
			showId,
			deskId,
			status: showId && deskId ? "loading" : "idle",
		};
		this.emit();
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
		if (!this.matches(projection.scope.show_id)) return false;
		const keys = projectionKeys(projection);
		const projections = new Map(this.state.projections);
		let changed = false;
		for (const key of keys) {
			if ((this.keySequences.get(key) ?? 0) >= sequence) continue;
			this.authoritativeProjections.set(
				key,
				upsertProjection(
					this.authoritativeProjections.get(key) ?? [],
					projection,
				),
			);
			this.keySequences.set(key, sequence);
			projections.set(key, this.renderProjectionKey(key));
			changed = true;
		}
		if (!changed) return false;
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
		const operation = this.takeProjectionOperation(token);
		const sequence = outcome.event_sequence ?? null;
		if (
			sequence != null ||
			!operation ||
			(this.keySequences.get(operation.key) ?? 0) <= operation.baseSequence
		)
			this.installAuthoritativeProjection(
				outcome.projection,
				sequence,
				operation?.baseSequence ?? this.state.eventSequence ?? 0,
			);
		if (outcome.desk) {
			const deskSequence = outcome.desk_event_sequence ?? sequence;
			if (deskSequence != null) this.applyDesk(outcome.desk, deskSequence);
			else
				this.publish({
					desk: this.deskState.installUnsequenced(
						outcome.desk,
						this.state.eventSequence ?? 0,
					),
				});
		}
		this.publish({ pendingKeys: this.pendingKeys(), error: null });
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
		const operation = {
			token: crypto.randomUUID(),
			key,
			baseSequence: this.keySequences.get(key) ?? 0,
			order: ++this.projectionOperationOrder,
			optimistic,
		};
		this.projectionOperations.set(operation.token, operation);
		this.latestProjectionTokens.set(key, operation.token);
		const projections = new Map(this.state.projections);
		projections.set(key, this.renderProjectionKey(key));
		this.publish({ projections, pendingKeys: this.pendingKeys(), error: null });
		return operation.token;
	}

	rollbackProjection(token: string | null, error: Error) {
		const operation = this.takeProjectionOperation(token);
		if (!operation) return;
		const projections = new Map(this.state.projections);
		projections.set(operation.key, this.renderProjectionKey(operation.key));
		this.publish({
			projections,
			pendingKeys: this.pendingKeys(),
			status: "error",
			error,
		});
	}

	beginOptimisticPage(page: number) {
		const operation = this.deskState.begin(page);
		if (!operation) return null;
		this.publish({ desk: operation.desk, error: null });
		return operation.token;
	}

	commitPage(token: string | null, _page: number, sequence: number | null) {
		const desk = this.deskState.commit(token, sequence);
		if (desk) this.publish({ desk, error: null });
	}

	rollbackPage(token: string | null, error: Error) {
		const desk = this.deskState.rollback(token);
		if (desk) this.publish({ desk, status: "error", error });
	}

	private installAuthoritativeProjection(
		projection: PlaybackProjection,
		sequence: number | null,
		baseSequence: number,
	) {
		if (sequence != null) {
			this.applyProjection(projection, sequence);
			return;
		}
		const projections = new Map(this.state.projections);
		for (const key of projectionKeys(projection)) {
			if ((this.keySequences.get(key) ?? 0) > baseSequence) continue;
			this.authoritativeProjections.set(
				key,
				upsertProjection(
					this.authoritativeProjections.get(key) ?? [],
					projection,
				),
			);
			projections.set(key, this.renderProjectionKey(key));
		}
		this.publish({ projections, status: "ready", error: null });
	}

	private takeProjectionOperation(token?: string | null) {
		if (!token) return null;
		const operation = this.projectionOperations.get(token);
		if (!operation) return null;
		this.projectionOperations.delete(token);
		if (this.latestProjectionTokens.get(operation.key) === token) {
			const latest = [...this.projectionOperations.values()]
				.filter((candidate) => candidate.key === operation.key)
				.sort((left, right) => right.order - left.order)[0];
			if (latest) this.latestProjectionTokens.set(operation.key, latest.token);
			else this.latestProjectionTokens.delete(operation.key);
		}
		return operation;
	}

	private renderProjectionKey(key: string) {
		const authoritative = this.authoritativeProjections.get(key) ?? [];
		const token = this.latestProjectionTokens.get(key);
		const optimistic = token
			? this.projectionOperations.get(token)?.optimistic
			: undefined;
		return optimistic
			? upsertProjection(authoritative, optimistic)
			: authoritative;
	}

	private pendingKeys() {
		return new Set(
			[...this.projectionOperations.values()].map((operation) => operation.key),
		);
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

function projectSnapshot(
	projections: readonly PlaybackProjection[],
	identities: readonly PlaybackIdentity[],
) {
	const result = new Map<string, PlaybackProjection[]>();
	for (const identity of identities) result.set(identityKey(identity), []);
	for (const projection of projections)
		for (const key of projectionKeys(projection))
			result.set(key, upsertProjection(result.get(key) ?? [], projection));
	return result;
}

function upsertProjection(
	current: readonly PlaybackProjection[],
	projection: PlaybackProjection,
) {
	const key = projection.playback_number ?? null;
	const next = current.filter((candidate) => candidate.playback_number !== key);
	next.push(projection);
	return next.sort(
		(left, right) => (left.playback_number ?? 0) - (right.playback_number ?? 0),
	);
}
