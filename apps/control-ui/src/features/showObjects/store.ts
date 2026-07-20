import type {
	ShowObject,
	ShowObjectBodies,
	ShowObjectCollections,
	ShowObjectKind,
	ShowObjectsChange,
} from "./contracts";
import {
	objectKey,
	sortObjects,
	upsertCollection,
} from "./storeProjection";
import { ShowObjectEventWatermarks } from "./eventWatermarks";
import type {
	CollectionUpdate,
	ShowObjectInstall,
	ShowObjectsSnapshot,
} from "./storeTypes";
import { ShowObjectPendingMutations } from "./pendingMutations";
import { applyAuthoritativeChange } from "./changeApplication";
import { installAuthoritativeObjects } from "./objectInstallation";
import {
	ALL_COLLECTIONS,
	createShowObjectsSnapshot,
	NO_COLLECTIONS,
	projectedCollection,
} from "./storeSnapshot";

export type { ShowObjectInstall, ShowObjectsSnapshot } from "./storeTypes";

export class ShowObjectsStore {
	private authoritative: ShowObjectCollections = { group: [], preset: [] };
	private authorityKey: string | null = null;
	private authorityGeneration = 0;
	private readonly pending = new ShowObjectPendingMutations();
	private readonly watermarks = new ShowObjectEventWatermarks();
	private readonly listeners = new Set<() => void>();
	private snapshot: ShowObjectsSnapshot = {
		showId: null,
		authorityGeneration: 0,
		showRevision: null,
		eventSequence: null,
		groups: [],
		presets: [],
		readyCollections: new Set(),
		pendingObjectKeys: new Set(),
		status: "idle",
		error: null,
	};

	readonly subscribe = (listener: () => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};

	readonly getSnapshot = () => this.snapshot;

	reset(showId: string | null, authorityKey?: string) {
		const authorityChanged =
			authorityKey !== undefined && authorityKey !== this.authorityKey;
		if (this.snapshot.showId === showId && !authorityChanged) return;
		if (authorityKey !== undefined) this.authorityKey = authorityKey;
		this.authorityGeneration += 1;
		this.authoritative = { group: [], preset: [] };
		this.pending.clear();
		this.watermarks.clear();
		this.publish({
			showId,
			authorityGeneration: this.authorityGeneration,
			showRevision: null,
			eventSequence: null,
			status: showId ? "loading" : "idle",
			readyCollections: new Set(),
			error: null,
		});
	}

	setCollection<K extends ShowObjectKind>(
		showId: string,
		kind: K,
		objects: ShowObjectCollections[K],
		eventFloor?: number,
	) {
		if (this.snapshot.showId !== showId) this.reset(showId);
		let next = [...objects] as ShowObjectCollections[K];
		if (eventFloor != null) {
			const currentIds = new Set(
				this.authoritative[kind].map((object) => object.id),
			);
			next = next.filter((object) => {
				const applied = this.watermarks.objectSequence(
					objectKey(kind, object.id),
				);
				return applied == null || applied <= eventFloor || currentIds.has(object.id);
			}) as ShowObjectCollections[K];
			for (const current of this.authoritative[kind]) {
				const key = objectKey(kind, current.id);
				const floor = this.watermarks.objectFloor(key);
				const applied = this.watermarks.objectSequence(key);
				if (
					(floor != null && floor > eventFloor) ||
					(applied != null && applied > eventFloor)
				)
					upsertCollection(next, current as ShowObjectCollections[K][number]);
			}
		}
		this.authoritative[kind] = next;
		this.watermarks.clearKind(kind, eventFloor);
		if (eventFloor != null) this.watermarks.setKindFloor(kind, eventFloor);
		this.publish(
			{
				readyCollections: new Set([...this.snapshot.readyCollections, kind]),
				status: "ready",
				error: null,
			},
			projectedCollection(kind),
		);
	}

	updateCollection<K extends ShowObjectKind>(
		kind: K,
		update: CollectionUpdate<K>,
	) {
		const current = [...this.authoritative[kind]] as ShowObjectCollections[K];
		const next = typeof update === "function" ? update(current) : update;
		this.authoritative[kind] = [...next] as ShowObjectCollections[K];
		this.publish({}, projectedCollection(kind));
	}

	installObject<K extends ShowObjectKind>(
		showId: string,
		kind: K,
		object: ShowObject<K> | null,
		minimumEventSequence?: number | null,
		absentObjectId?: string,
	) {
		const objectId = object?.id ?? absentObjectId;
		if (!objectId) return;
		this.installObjects(
			showId,
			[{ kind, objectId, object: object as ShowObject | null }],
			minimumEventSequence,
		);
	}

	/** Installs one authoritative exact-object projection (and its dependencies) atomically. */
	installObjects(
		showId: string,
		installs: readonly ShowObjectInstall[],
		minimumEventSequence?: number | null,
	) {
		if (this.snapshot.showId !== showId) return;
		const projectKinds = installAuthoritativeObjects(
			this.authoritative,
			this.watermarks,
			installs,
			minimumEventSequence,
		);
		this.publish({ status: "ready", error: null }, projectKinds);
	}

	beginOptimistic<K extends ShowObjectKind>(
		showId: string,
		kind: K,
		objectId: string,
		body: ShowObjectBodies[K],
	) {
		return this.addPending(showId, kind, objectId, body);
	}

	beginPending<K extends ShowObjectKind>(
		showId: string,
		kind: K,
		objectId: string,
	) {
		return this.addPending(showId, kind, objectId, null);
	}

	commit(
		token: string,
		objectRevision: number,
		minimumEventSequence?: number | null,
	) {
		const operation = this.pending.take(token);
		if (!operation || this.snapshot.showId !== operation.showId) return;
		const key = objectKey(operation.kind, operation.objectId);
		this.watermarks.raiseObjectFloor(
			operation.kind,
			operation.objectId,
			minimumEventSequence,
		);
		const existing = this.authoritative[operation.kind].find(
			(candidate) => candidate.id === operation.objectId,
		);
		const eventSequence = this.watermarks.appliedSequence(operation.kind, key);
		const responseEventObserved =
			minimumEventSequence != null
				? eventSequence >= minimumEventSequence
				: eventSequence > operation.baseEventSequence;
		if (
			(!existing || existing.revision < objectRevision) &&
			!responseEventObserved
		) {
			this.upsertAuthoritative(operation.kind, {
				kind: operation.kind,
				id: operation.objectId,
				revision: objectRevision,
				updated_at: existing?.updated_at ?? "",
				body: operation.body,
			} as ShowObject);
		}
		this.publish({ error: null }, projectedCollection(operation.kind));
	}

	rollback(token: string, error: Error) {
		const operation = this.pending.take(token);
		if (!operation) return;
		this.publish(
			{ status: "error", error },
			operation.body == null
				? NO_COLLECTIONS
				: projectedCollection(operation.kind),
		);
	}

	abandon(token: string) {
		const operation = this.pending.take(token);
		if (!operation) return;
		this.publish(
			{},
			operation.body == null
				? NO_COLLECTIONS
				: projectedCollection(operation.kind),
		);
	}

	settlePending<K extends ShowObjectKind>(
		token: string,
		object: ShowObject<K>,
		showRevision: number,
		minimumEventSequence: number | null,
		authorityGeneration: number,
	) {
		const operation = this.pending.take(token);
		if (
			!operation ||
			this.snapshot.showId !== operation.showId ||
			this.authorityGeneration !== authorityGeneration
		)
			return false;
		const key = objectKey(operation.kind, object.id);
		const responseEventObserved = this.hasAppliedAtOrAfter(
			operation.kind,
			key,
			minimumEventSequence,
		);
		this.watermarks.raiseObjectFloor(
			operation.kind,
			object.id,
			minimumEventSequence,
		);
		if (!responseEventObserved) {
			const existing = this.authoritative[operation.kind].find(
				(candidate) => candidate.id === object.id,
			);
			if (!existing || existing.revision <= object.revision)
				this.upsertAuthoritative(operation.kind, object);
		}
		this.publish(
			{
				showRevision: Math.max(this.snapshot.showRevision ?? 0, showRevision),
				status: "ready",
				error: null,
			},
			projectedCollection(operation.kind),
		);
		return true;
	}

	applyChange(change: ShowObjectsChange) {
		if (change.showId !== this.snapshot.showId) return;
		const applied = applyAuthoritativeChange(
			this.authoritative,
			this.watermarks,
			change,
		);
		if (!applied.accepted) return;
		this.publish(
			{
				showRevision: Math.max(
					this.snapshot.showRevision ?? 0,
					change.showRevision,
				),
				eventSequence: Math.max(
					this.snapshot.eventSequence ?? 0,
					change.eventSequence,
				),
				status: "ready",
				error: null,
			},
			applied.projectKinds,
		);
	}

	setLoading(kind?: ShowObjectKind) {
		if (!kind) return this.publish({ status: "loading" }, NO_COLLECTIONS);
		const readyCollections = new Set(this.snapshot.readyCollections);
		readyCollections.delete(kind);
		this.publish({ readyCollections, status: "loading" }, NO_COLLECTIONS);
	}

	isCollectionReady(kind: ShowObjectKind) {
		return this.snapshot.readyCollections.has(kind);
	}

	setError(error: Error) {
		this.publish({ status: "error", error }, NO_COLLECTIONS);
	}

	setReady() {
		this.publish({ status: "ready", error: null }, NO_COLLECTIONS);
	}

	beginEventResync() {
		this.watermarks.clear();
		this.publish(
			{ showRevision: null, eventSequence: null },
			NO_COLLECTIONS,
		);
	}

	private addPending<K extends ShowObjectKind>(
		showId: string,
		kind: K,
		objectId: string,
		body: ShowObjectBodies[K] | null,
	) {
		if (this.snapshot.showId !== showId)
			throw new Error(`Show ${showId} is no longer active`);
		const key = objectKey(kind, objectId);
		const token = this.pending.begin(
			showId,
			kind,
			objectId,
			body,
			this.watermarks.appliedSequence(kind, key),
		);
		this.publish(
			{ error: null },
			body == null ? NO_COLLECTIONS : projectedCollection(kind),
		);
		return token;
	}

	private upsertAuthoritative(kind: ShowObjectKind, object: ShowObject) {
		const objects = this.authoritative[kind] as ShowObject[];
		const index = objects.findIndex((candidate) => candidate.id === object.id);
		if (index < 0) objects.push(object);
		else objects[index] = object;
		sortObjects(objects);
	}

	private hasAppliedAtOrAfter(
		kind: ShowObjectKind,
		key: string,
		minimumEventSequence?: number | null,
	) {
		return this.watermarks.hasAppliedAtOrAfter(
			kind,
			key,
			minimumEventSequence,
		);
	}

	private publish(
		changes: Partial<ShowObjectsSnapshot> = {},
		projectKinds: ReadonlySet<ShowObjectKind> = ALL_COLLECTIONS,
	) {
		this.snapshot = createShowObjectsSnapshot(
			this.snapshot,
			this.authoritative,
			this.pending.values(),
			this.pending.keys(),
			changes,
			projectKinds,
		);
		for (const listener of this.listeners) listener();
	}
}
