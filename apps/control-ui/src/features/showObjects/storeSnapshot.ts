import type {
	ShowObject,
	ShowObjectCollections,
	ShowObjectKind,
} from "./contracts";
import { projectCollection } from "./storeProjection";
import type { PendingMutation, ShowObjectsSnapshot } from "./storeTypes";

export const ALL_COLLECTIONS = new Set<ShowObjectKind>(["group", "preset"]);
export const NO_COLLECTIONS = new Set<ShowObjectKind>();
const GROUP_COLLECTION = new Set<ShowObjectKind>(["group"]);
const PRESET_COLLECTION = new Set<ShowObjectKind>(["preset"]);

export function projectedCollection(kind: ShowObjectKind) {
	return kind === "group" ? GROUP_COLLECTION : PRESET_COLLECTION;
}

export function createShowObjectsSnapshot(
	previous: ShowObjectsSnapshot,
	authoritative: ShowObjectCollections,
	pending: Iterable<PendingMutation[]>,
	pendingKeys: Iterable<string>,
	changes: Partial<ShowObjectsSnapshot>,
	projectKinds: ReadonlySet<ShowObjectKind>,
): ShowObjectsSnapshot {
	const pendingOperations = projectKinds.size > 0 ? [...pending] : [];
	return {
		...previous,
		groups: projectKinds.has("group")
			? (projectCollection(
					"group",
					authoritative.group,
					pendingOperations,
				) as ShowObject<"group">[])
			: previous.groups,
		presets: projectKinds.has("preset")
			? (projectCollection(
					"preset",
					authoritative.preset,
					pendingOperations,
				) as ShowObject<"preset">[])
			: previous.presets,
		pendingObjectKeys: new Set(pendingKeys),
		...changes,
	};
}
