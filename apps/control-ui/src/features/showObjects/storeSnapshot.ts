import type {
	ShowObject,
	ShowObjectCollections,
	ShowObjectKind,
} from "./contracts";
import { projectCollection } from "./storeProjection";
import type { PendingMutation, ShowObjectsSnapshot } from "./storeTypes";

export const ALL_COLLECTIONS = new Set<ShowObjectKind>([
	"group",
	"preset",
	"cue_list",
	"playback",
	"playback_page",
]);
export const NO_COLLECTIONS = new Set<ShowObjectKind>();
const PROJECTED_COLLECTIONS: Record<ShowObjectKind, ReadonlySet<ShowObjectKind>> = {
	group: new Set(["group"]),
	preset: new Set(["preset"]),
	cue_list: new Set(["cue_list"]),
	playback: new Set(["playback"]),
	playback_page: new Set(["playback_page"]),
};

export function emptyShowObjectCollections(): ShowObjectCollections {
	return {
		group: [],
		preset: [],
		cue_list: [],
		playback: [],
		playback_page: [],
	};
}

export function initialShowObjectsSnapshot(): ShowObjectsSnapshot {
	return {
		showId: null,
		authorityGeneration: 0,
		showRevision: null,
		eventSequence: null,
		groups: [],
		presets: [],
		cueLists: [],
		playbacks: [],
		playbackPages: [],
		readyCollections: new Set(),
		pendingObjectKeys: new Set(),
		status: "idle",
		error: null,
	};
}

export function projectedCollection(kind: ShowObjectKind) {
	return PROJECTED_COLLECTIONS[kind];
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
		cueLists: projectKinds.has("cue_list")
			? (projectCollection(
					"cue_list",
					authoritative.cue_list,
					pendingOperations,
				) as ShowObject<"cue_list">[])
			: previous.cueLists,
		playbacks: projectKinds.has("playback")
			? (projectCollection(
					"playback",
					authoritative.playback,
					pendingOperations,
				) as ShowObject<"playback">[])
			: previous.playbacks,
		playbackPages: projectKinds.has("playback_page")
			? (projectCollection(
					"playback_page",
					authoritative.playback_page,
					pendingOperations,
				) as ShowObject<"playback_page">[])
			: previous.playbackPages,
		pendingObjectKeys: new Set(pendingKeys),
		...changes,
	};
}
