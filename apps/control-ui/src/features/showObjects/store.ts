import type {
	ShowObject,
	ShowObjectBodies,
	ShowObjectCollections,
	ShowObjectKind,
	ShowObjectsChange,
} from "./contracts";
import {
	objectKey,
	projectCollection,
	sortObjects,
	upsertCollection,
} from "./storeProjection";
import type {
	CollectionUpdate,
	PendingMutation,
	ShowObjectInstall,
	ShowObjectsSnapshot,
} from "./storeTypes";

export type { ShowObjectInstall, ShowObjectsSnapshot } from "./storeTypes";

export class ShowObjectsStore {
	private authoritative: ShowObjectCollections = { group: [], preset: [] };
	private readonly pending = new Map<string, PendingMutation[]>();
	private readonly objectSequences = new Map<string, number>();
	private readonly objectSequenceFloors = new Map<string, number>();
	private readonly kindSequenceFloors = new Map<ShowObjectKind, number>();
	private readonly listeners = new Set<() => void>();
	private snapshot: ShowObjectsSnapshot = {
		showId: null,
		showRevision: null,
		eventSequence: null,
		groups: [],
		presets: [],
		pendingObjectKeys: new Set(),
		status: "idle",
		error: null,
	};

	readonly subscribe = (listener: () => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};

	readonly getSnapshot = () => this.snapshot;

	reset(showId: string | null) {
		if (this.snapshot.showId === showId) return;
		this.authoritative = { group: [], preset: [] };
		this.pending.clear();
		this.clearEventWatermarks();
		this.publish({
			showId,
			showRevision: null,
			eventSequence: null,
			status: showId ? "loading" : "idle",
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
				const applied = this.objectSequences.get(objectKey(kind, object.id));
				return applied == null || applied <= eventFloor || currentIds.has(object.id);
			}) as ShowObjectCollections[K];
			for (const current of this.authoritative[kind]) {
				const key = objectKey(kind, current.id);
				const floor = this.objectSequenceFloors.get(key);
				const applied = this.objectSequences.get(key);
				if (
					(floor != null && floor > eventFloor) ||
					(applied != null && applied > eventFloor)
				)
					upsertCollection(next, current as ShowObjectCollections[K][number]);
			}
		}
		this.authoritative[kind] = next;
		this.clearKindWatermarks(kind, eventFloor);
		if (eventFloor != null) this.kindSequenceFloors.set(kind, eventFloor);
		this.publish({ status: "ready", error: null });
	}

	updateCollection<K extends ShowObjectKind>(
		kind: K,
		update: CollectionUpdate<K>,
	) {
		const current = [...this.authoritative[kind]] as ShowObjectCollections[K];
		const next = typeof update === "function" ? update(current) : update;
		this.authoritative[kind] = [...next] as ShowObjectCollections[K];
		this.publish();
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
		for (const { kind, objectId, object } of installs) {
			const key = objectKey(kind, objectId);
			const responseEventObserved = this.hasAppliedAtOrAfter(
				kind,
				key,
				minimumEventSequence,
			);
			this.raiseObjectSequenceFloor(kind, objectId, minimumEventSequence);
			const existing = this.authoritative[kind].find(
				(candidate) => candidate.id === objectId,
			);
			if (!responseEventObserved && !object)
				this.removeAuthoritative(kind, objectId);
			else if (
				!responseEventObserved &&
				object &&
				(minimumEventSequence != null ||
					!existing ||
					existing.revision <= object.revision)
			)
				this.upsertAuthoritative(kind, object);
		}
		this.publish({ status: "ready", error: null });
	}

	beginOptimistic<K extends ShowObjectKind>(
		showId: string,
		kind: K,
		objectId: string,
		body: ShowObjectBodies[K],
	) {
		if (this.snapshot.showId !== showId)
			throw new Error(`Show ${showId} is no longer active`);
		const token = crypto.randomUUID();
		const key = objectKey(kind, objectId);
		const operations = this.pending.get(key) ?? [];
		operations.push({
			token,
			showId,
			kind,
			objectId,
			body,
			baseEventSequence: this.appliedSequence(kind, key),
		});
		this.pending.set(key, operations);
		this.publish({ error: null });
		return token;
	}

	commit(
		token: string,
		objectRevision: number,
		minimumEventSequence?: number | null,
	) {
		const operation = this.takePending(token);
		if (!operation || this.snapshot.showId !== operation.showId) return;
		const key = objectKey(operation.kind, operation.objectId);
		this.raiseObjectSequenceFloor(
			operation.kind,
			operation.objectId,
			minimumEventSequence,
		);
		const existing = this.authoritative[operation.kind].find(
			(candidate) => candidate.id === operation.objectId,
		);
		const eventSequence = this.appliedSequence(operation.kind, key);
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
		this.publish({ error: null });
	}

	rollback(token: string, error: Error) {
		if (!this.takePending(token)) return;
		this.publish({ status: "error", error });
	}

	applyChange(change: ShowObjectsChange) {
		if (change.showId !== this.snapshot.showId) return;
		let changed = false;
		for (const objectChange of change.changes) {
			const key = objectKey(objectChange.kind, objectChange.objectId);
			const existing = this.authoritative[objectChange.kind].find(
				(candidate) => candidate.id === objectChange.objectId,
			);
			const kindFloor = this.kindSequenceFloors.get(objectChange.kind) ?? 0;
			const objectFloor = this.objectSequenceFloors.get(key) ?? 0;
			const applied = this.objectSequences.get(key) ?? 0;
			if (
				change.eventSequence <= kindFloor ||
				change.eventSequence < objectFloor ||
				change.eventSequence <= applied
			)
				continue;
			this.objectSequences.set(key, change.eventSequence);
			if (change.eventSequence >= objectFloor)
				this.objectSequenceFloors.delete(key);
			changed = true;
			if (existing && existing.revision > objectChange.objectRevision) continue;
			if (objectChange.deleted) {
				this.removeAuthoritative(objectChange.kind, objectChange.objectId);
			} else if (objectChange.body) {
				this.upsertAuthoritative(objectChange.kind, {
					kind: objectChange.kind,
					id: objectChange.objectId,
					revision: objectChange.objectRevision,
					updated_at: existing?.updated_at ?? "",
					body: objectChange.body,
				} as ShowObject);
			}
		}
		if (!changed) return;
		this.publish({
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
		});
	}

	setLoading() {
		this.publish({ status: "loading" });
	}

	setError(error: Error) {
		this.publish({ status: "error", error });
	}

	setReady() {
		this.publish({ status: "ready", error: null });
	}

	beginEventResync() {
		this.clearEventWatermarks();
		this.publish({ showRevision: null, eventSequence: null });
	}

	private upsertAuthoritative(kind: ShowObjectKind, object: ShowObject) {
		const objects = this.authoritative[kind] as ShowObject[];
		const index = objects.findIndex((candidate) => candidate.id === object.id);
		if (index < 0) objects.push(object);
		else objects[index] = object;
		sortObjects(objects);
	}

	private removeAuthoritative(kind: ShowObjectKind, objectId: string) {
		this.authoritative[kind] = this.authoritative[kind].filter(
			(object) => object.id !== objectId,
		) as never;
	}

	private takePending(token: string): PendingMutation | null {
		for (const [key, operations] of this.pending) {
			const index = operations.findIndex((operation) => operation.token === token);
			if (index < 0) continue;
			const [operation] = operations.splice(index, 1);
			if (!operations.length) this.pending.delete(key);
			return operation;
		}
		return null;
	}

	private appliedSequence(kind: ShowObjectKind, key: string) {
		return Math.max(
			this.kindSequenceFloors.get(kind) ?? 0,
			this.objectSequences.get(key) ?? 0,
		);
	}

	private hasAppliedAtOrAfter(
		kind: ShowObjectKind,
		key: string,
		minimumEventSequence?: number | null,
	) {
		if (minimumEventSequence == null) return false;
		return (
			(this.kindSequenceFloors.get(kind) ?? -1) >= minimumEventSequence ||
			(this.objectSequences.get(key) ?? -1) >= minimumEventSequence
		);
	}

	private raiseObjectSequenceFloor(
		kind: ShowObjectKind,
		objectId: string,
		minimumEventSequence?: number | null,
	) {
		if (minimumEventSequence == null) return;
		const key = objectKey(kind, objectId);
		if (minimumEventSequence <= this.appliedSequence(kind, key)) return;
		this.objectSequenceFloors.set(
			key,
			Math.max(this.objectSequenceFloors.get(key) ?? 0, minimumEventSequence),
		);
	}

	private clearKindWatermarks(kind: ShowObjectKind, eventFloor?: number) {
		const prefix = `${kind}:`;
		for (const [key, sequence] of this.objectSequences)
			if (
				key.startsWith(prefix) &&
				(eventFloor == null || sequence <= eventFloor)
			)
				this.objectSequences.delete(key);
		for (const [key, sequence] of this.objectSequenceFloors)
			if (
				key.startsWith(prefix) &&
				(eventFloor == null || sequence <= eventFloor)
			)
				this.objectSequenceFloors.delete(key);
		this.kindSequenceFloors.delete(kind);
	}

	private clearEventWatermarks() {
		this.objectSequences.clear();
		this.objectSequenceFloors.clear();
		this.kindSequenceFloors.clear();
	}

	private createSnapshot(
		changes: Partial<ShowObjectsSnapshot> = {},
	): ShowObjectsSnapshot {
		return {
			...this.snapshot,
			groups: projectCollection(
				"group",
				this.authoritative.group,
				this.pending.values(),
			) as ShowObject<"group">[],
			presets: projectCollection(
				"preset",
				this.authoritative.preset,
				this.pending.values(),
			) as ShowObject<"preset">[],
			pendingObjectKeys: new Set(this.pending.keys()),
			...changes,
		};
	}

	private publish(changes: Partial<ShowObjectsSnapshot> = {}) {
		this.snapshot = this.createSnapshot(changes);
		for (const listener of this.listeners) listener();
	}
}
