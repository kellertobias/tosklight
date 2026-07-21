import type {
	CueList,
	OutputRoute,
	PatchLayer,
	ServerEvent,
	SessionResponse,
	VersionedObject,
} from "../../api/types";
import type { StoredDeskLayout, StoredStageLayout } from "./contracts";
import type { ServerState } from "./useServerState";

type ServerStateSource = () => ServerState;

type SingleObjectKind =
	| "cue_list"
	| "patch_layer"
	| "stage_layout"
	| "unresolved_mvr_fixture"
	| "user_layout";

interface ObjectChange {
	sequence: number;
	showId: string;
	kind: string;
	id: string;
	objectRevision: number;
	deleted: boolean;
}

interface ReconciliationTask {
	key: string;
	sequence: number;
	run: (isCurrent: () => boolean) => Promise<void>;
}

const singleObjectKinds = new Set<SingleObjectKind>([
	"cue_list",
	"patch_layer",
	"stage_layout",
	"unresolved_mvr_fixture",
	"user_layout",
]);

class LatestReconciliationQueue {
	private readonly latestSequence = new Map<string, number>();
	private readonly pending = new Map<string, ReconciliationTask>();
	private readonly draining = new Set<string>();

	enqueue(task: ReconciliationTask) {
		const latest = this.latestSequence.get(task.key);
		if (latest != null && task.sequence <= latest) return;
		this.latestSequence.set(task.key, task.sequence);
		this.pending.set(task.key, task);
		if (this.draining.has(task.key)) return;
		this.draining.add(task.key);
		queueMicrotask(() => void this.drain(task.key));
	}

	private async drain(key: string) {
		try {
			for (;;) {
				const task = this.pending.get(key);
				if (!task) return;
				this.pending.delete(key);
				const isCurrent = () => this.latestSequence.get(key) === task.sequence;
				await task.run(isCurrent).catch(() => undefined);
			}
		} finally {
			this.draining.delete(key);
		}
	}
}

function activeShowId(state: ServerState) {
	return state.bootstrap?.active_show?.id ?? null;
}

function stringField(payload: Record<string, unknown>, field: string) {
	const value = payload[field];
	return typeof value === "string" && value ? value : null;
}

function revisionField(payload: Record<string, unknown>) {
	const value = payload.revision;
	return Number.isSafeInteger(value) && (value as number) >= 0
		? (value as number)
		: null;
}

function showObjectChange(event: ServerEvent): ObjectChange | null {
	if (event.kind !== "show_object_changed") return null;
	const showId = stringField(event.payload, "show_id");
	const kind = stringField(event.payload, "kind");
	const id = stringField(event.payload, "id");
	const objectRevision = revisionField(event.payload);
	if (!showId || !kind || !id || objectRevision == null) return null;
	return {
		sequence: event.revision,
		showId,
		kind,
		id,
		objectRevision,
		deleted: event.payload.deleted === true,
	};
}

function preloadObjectChange(
	event: ServerEvent,
	state: ServerState,
): ObjectChange | null {
	if (event.kind !== "preload_stored") return null;
	const target = stringField(event.payload, "target");
	const id = stringField(event.payload, "target_id");
	const objectRevision = revisionField(event.payload);
	const showId = activeShowId(state);
	const kind = target === "cue" ? "cue_list" : null;
	if (!showId || !kind || !id || objectRevision == null) return null;
	return {
		sequence: event.revision,
		showId,
		kind,
		id,
		objectRevision,
		deleted: false,
	};
}

function isSingleObjectKind(kind: string): kind is SingleObjectKind {
	return singleObjectKinds.has(kind as SingleObjectKind);
}

function validObject<T>(
	change: ObjectChange,
	object: VersionedObject<T>,
): object is VersionedObject<T> {
	return (
		object.id === change.id &&
		object.kind === change.kind &&
		object.revision >= change.objectRevision
	);
}

function reconcileList<T>(
	current: VersionedObject<T>[],
	change: ObjectChange,
	object: VersionedObject<T> | null,
) {
	const index = current.findIndex((candidate) => candidate.id === change.id);
	const existing = index >= 0 ? current[index] : null;
	if (change.deleted) {
		if (!existing) return current;
		return current.filter((candidate) => candidate.id !== change.id);
	}
	if (!object || !validObject(change, object)) return current;
	if (!existing)
		return [...current, object].sort((left, right) =>
			left.id < right.id ? -1 : left.id > right.id ? 1 : 0,
		);
	if (existing === object) return current;
	const next = [...current];
	next[index] = object;
	return next;
}

function reconcileSingle<T>(
	current: VersionedObject<T> | null,
	change: ObjectChange,
	object: VersionedObject<T> | null,
) {
	if (change.deleted) {
		return current?.id === change.id ? null : current;
	}
	if (!object || !validObject(change, object)) return current;
	return current === object ? current : object;
}

function defaultPatchLayer(): VersionedObject<PatchLayer> {
	return {
		kind: "patch_layer",
		id: "default",
		revision: 0,
		updated_at: "",
		body: { id: "default", name: "Default", order: 0 },
	};
}

function installObject(
	state: ServerState,
	change: ObjectChange,
	object: VersionedObject<unknown> | null,
) {
	if (!change.deleted && (!object || !validObject(change, object))) return;
	switch (change.kind as SingleObjectKind) {
		case "cue_list":
			state.setCueObjects((current) =>
				reconcileList(
					current,
					change,
					object as VersionedObject<CueList> | null,
				),
			);
			return;
		case "patch_layer":
			state.setPatchLayers((current) => {
				const stored = current.filter(
					(layer) =>
						!(
							layer.id === "default" &&
							layer.revision === 0 &&
							layer.updated_at === ""
						),
				);
				const next = reconcileList(
					stored,
					change,
					object as VersionedObject<PatchLayer> | null,
				);
				return next.length ? next : [defaultPatchLayer()];
			});
			return;
		case "unresolved_mvr_fixture":
			state.setUnresolvedMvrFixtures((current) =>
				reconcileList(
					current,
					change,
					object as VersionedObject<Record<string, unknown>> | null,
				),
			);
			return;
		case "user_layout":
			state.setDeskLayout((current) =>
				reconcileSingle(
					current,
					change,
					object as VersionedObject<StoredDeskLayout> | null,
				),
			);
			return;
		case "stage_layout":
			state.setStageLayout((current) =>
				reconcileSingle(
					current,
					change,
					object as VersionedObject<StoredStageLayout> | null,
				),
			);
	}
}

function objectKey(change: ObjectChange) {
	return `show:${change.showId}:object:${change.kind}:${change.id}`;
}

function projectionKey(change: ObjectChange, projection: string) {
	return `show:${change.showId}:projection:${projection}`;
}

function enqueueRouteProjection(
	queue: LatestReconciliationQueue,
	getState: ServerStateSource,
	change: ObjectChange,
) {
	queue.enqueue({
		key: projectionKey(change, "routes"),
		sequence: change.sequence,
		run: async (isCurrent) => {
			const routes = await getState().client.objects<OutputRoute>(
				change.showId,
				"route",
			);
			const state = getState();
			if (!isCurrent() || activeShowId(state) !== change.showId) return;
			state.setOutputRoutes(routes);
			state.setPatch((current) =>
				current
					? { ...current, routes: routes.map((route) => route.body) }
					: current,
			);
		},
	});
}

export function createShowObjectEventReconciler(
	getState: ServerStateSource,
	session: SessionResponse,
) {
	const queue = new LatestReconciliationQueue();

	const enqueueObject = (change: ObjectChange) => {
		if (activeShowId(getState()) !== change.showId) return;
		if (change.kind === "user_layout" && change.id !== session.user.id) return;
		if (change.kind === "stage_layout" && change.id !== "main") return;
		if (change.kind === "route") {
			enqueueRouteProjection(queue, getState, change);
			return;
		}
		if (change.kind === "patched_fixture") {
			queue.enqueue({
				key: projectionKey(change, "patch"),
				sequence: change.sequence,
				run: async (isCurrent) => {
					const next = await getState().client.patch();
					const state = getState();
					if (isCurrent() && activeShowId(state) === change.showId)
						state.setPatch(next);
				},
			});
			return;
		}
		if (!isSingleObjectKind(change.kind)) return;
		queue.enqueue({
			key: objectKey(change),
			sequence: change.sequence,
			run: async (isCurrent) => {
				const object = change.deleted
					? null
					: await getState().client.object<unknown>(
							change.showId,
							change.kind,
							change.id,
						);
				const state = getState();
				if (!isCurrent() || activeShowId(state) !== change.showId) return;
				installObject(state, change, object);
			},
		});
	};

	return (event: ServerEvent) => {
		const state = getState();
		const change = showObjectChange(event) ?? preloadObjectChange(event, state);
		if (change) enqueueObject(change);
	};
}
