import type { ShowObject, ShowObjectKind } from "./contracts";
import type { ShowObjectInstall } from "./storeTypes";
import type { HydrationTarget } from "./scope";

export type ShowObjectCollectionLoader = (
	showId: string,
	kind: ShowObjectKind,
) => Promise<ShowObject[]>;

export type ShowObjectLoader = (
	showId: string,
	kind: ShowObjectKind,
	objectId: string,
) => Promise<ShowObject | null>;

export interface HydrationLoad {
	collection: ShowObject[] | null;
	installs: ShowObjectInstall[];
	groupDependencies: ReadonlySet<string> | null;
}

export async function loadHydration(
	target: HydrationTarget,
	showId: string,
	loadCollection: ShowObjectCollectionLoader,
	loadObject: ShowObjectLoader,
): Promise<HydrationLoad> {
	if (target.objectId === undefined)
		return {
			collection: await loadCollection(showId, target.kind),
			installs: [],
			groupDependencies: null,
		};
	if (target.kind !== "group") {
		const object = await loadExact(loadObject, showId, target.kind, target.objectId);
		return {
			collection: null,
			installs: [{ kind: target.kind, objectId: target.objectId, object }],
			groupDependencies: null,
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
	let objectId: string | null = targetId;
	while (objectId && !visited.has(objectId)) {
		visited.add(objectId);
		const object: ShowObject<"group"> | null = await loadExact(
			loadObject,
			showId,
			"group",
			objectId,
		);
		installs.push({ kind: "group", objectId, object });
		if (!object) break;
		const sourceId: string | null =
			object.body.derived_from?.source_group_id ?? null;
		if (!sourceId) break;
		dependencies.add(sourceId);
		objectId = sourceId;
	}
	return {
		collection: null,
		installs,
		groupDependencies: dependencies,
	};
}

async function loadExact<K extends ShowObjectKind>(
	loadObject: ShowObjectLoader,
	showId: string,
	kind: K,
	objectId: string,
): Promise<ShowObject<K> | null> {
	const object = await loadObject(showId, kind, objectId);
	if (object && (object.kind !== kind || object.id !== objectId))
		throw new Error(
			`Expected ${kind} ${objectId}, received ${object.kind} ${object.id}`,
		);
	return object as ShowObject<K> | null;
}
