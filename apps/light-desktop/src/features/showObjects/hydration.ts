import type { ShowObject, ShowObjectKind } from "./contracts";
import { groupReferenceIds } from "./groupProjection";
import type { HydrationTarget } from "./scope";
import type { ShowObjectInstall } from "./storeTypes";

export type ShowObjectCollectionLoader = (
	showId: string,
	kind: ShowObjectKind,
) => Promise<{
	objects: ShowObject[];
	showRevision: number;
}>;

export type ShowObjectLoader = (
	showId: string,
	kind: ShowObjectKind,
	objectId: string,
) => Promise<{
	object: ShowObject | null;
	showRevision: number;
}>;

export interface HydrationLoad {
	collection: ShowObject[] | null;
	installs: ShowObjectInstall[];
	groupDependencies: ReadonlySet<string> | null;
	showRevision: number;
}

export async function loadHydration(
	target: HydrationTarget,
	showId: string,
	loadCollection: ShowObjectCollectionLoader,
	loadObject: ShowObjectLoader,
): Promise<HydrationLoad> {
	if (target.objectId === undefined) {
		const snapshot = await loadCollection(showId, target.kind);
		return {
			collection: snapshot.objects,
			installs: [],
			groupDependencies: null,
			showRevision: snapshot.showRevision,
		};
	}
	if (target.kind !== "group") {
		const snapshot = await loadExact(
			loadObject,
			showId,
			target.kind,
			target.objectId,
		);
		return {
			collection: null,
			installs: [
				{
					kind: target.kind,
					objectId: target.objectId,
					object: snapshot.object,
				},
			],
			groupDependencies: null,
			showRevision: snapshot.showRevision,
		};
	}
	return loadGroupGraph(showId, target.objectId, loadObject);
}

async function loadGroupGraph(
	showId: string,
	targetId: string,
	loadObject: ShowObjectLoader,
): Promise<HydrationLoad> {
	const installs: ShowObjectInstall[] = [];
	const dependencies = new Set<string>();
	const visited = new Set<string>();
	let showRevision: number | null = null;
	const pending = [targetId];
	while (pending.length > 0) {
		const objectId = pending.shift();
		if (!objectId || visited.has(objectId)) continue;
		visited.add(objectId);
		const snapshot = await loadExact(loadObject, showId, "group", objectId);
		showRevision = consistentRevision(showRevision, snapshot.showRevision);
		const object = snapshot.object as ShowObject<"group"> | null;
		installs.push({ kind: "group", objectId, object });
		if (!object) continue;
		for (const sourceId of groupReferenceIds(object.body)) {
			dependencies.add(sourceId);
			if (!visited.has(sourceId)) pending.push(sourceId);
		}
	}
	if (showRevision == null)
		throw new Error("Exact Group hydration requires a non-empty object ID");
	return {
		collection: null,
		installs,
		groupDependencies: dependencies,
		showRevision,
	};
}

async function loadExact<K extends ShowObjectKind>(
	loadObject: ShowObjectLoader,
	showId: string,
	kind: K,
	objectId: string,
): Promise<{ object: ShowObject<K> | null; showRevision: number }> {
	const snapshot = await loadObject(showId, kind, objectId);
	const object = snapshot.object;
	if (object && (object.kind !== kind || object.id !== objectId))
		throw new Error(
			`Expected ${kind} ${objectId}, received ${object.kind} ${object.id}`,
		);
	return {
		object: object as ShowObject<K> | null,
		showRevision: snapshot.showRevision,
	};
}

function consistentRevision(current: number | null, next: number) {
	if (current == null || current === next) return next;
	throw new Error("Show changed while hydrating the exact Group graph");
}
