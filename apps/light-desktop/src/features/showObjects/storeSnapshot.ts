import type {
	ShowObject,
	ShowObjectCollections,
	ShowObjectKind,
} from "./contracts";
import { projectCollection } from "./storeProjection";
import type { PendingMutation, ShowObjectsSnapshot } from "./storeTypes";

export const ALL_COLLECTIONS = new Set<ShowObjectKind>([
	"macro",
	"timecode",
	"dynamic",
	"group",
	"preset",
	"cue_list",
	"patch_layer",
	"playback_page",
	"stage_layout",
	"user_layout",
]);
export const NO_COLLECTIONS = new Set<ShowObjectKind>();
const PROJECTED_COLLECTIONS: Record<
	ShowObjectKind,
	ReadonlySet<ShowObjectKind>
> = {
	macro: new Set(["macro"]),
	timecode: new Set(["timecode"]),
	dynamic: new Set(["dynamic"]),
	group: new Set(["group"]),
	preset: new Set(["preset"]),
	cue_list: new Set(["cue_list"]),
	patch_layer: new Set(["patch_layer"]),
	playback_page: new Set(["playback_page"]),
	stage_layout: new Set(["stage_layout"]),
	user_layout: new Set(["user_layout"]),
};

export function emptyShowObjectCollections(): ShowObjectCollections {
	return {
		macro: [],
		timecode: [],
		dynamic: [],
		group: [],
		preset: [],
		cue_list: [],
		patch_layer: [],
		playback_page: [],
		stage_layout: [],
		user_layout: [],
	};
}

export function initialShowObjectsSnapshot(): ShowObjectsSnapshot {
	return {
		showId: null,
		authorityGeneration: 0,
		showRevision: null,
		eventSequence: null,
		macros: [],
		timecodes: [],
		dynamics: [],
		groups: [],
		presets: [],
		cueLists: [],
		patchLayers: [],
		playbackPages: [],
		stageLayouts: [],
		userLayouts: [],
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
		macros: projectKinds.has("macro")
			? (projectCollection(
					"macro",
					authoritative.macro,
					pendingOperations,
				) as ShowObject<"macro">[])
			: previous.macros,
		timecodes: projectKinds.has("timecode")
			? (projectCollection(
					"timecode",
					authoritative.timecode,
					pendingOperations,
				) as ShowObject<"timecode">[])
			: previous.timecodes,
		dynamics: projectKinds.has("dynamic")
			? (projectCollection(
					"dynamic",
					authoritative.dynamic,
					pendingOperations,
				) as ShowObject<"dynamic">[])
			: previous.dynamics,
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
		patchLayers: projectKinds.has("patch_layer")
			? (projectCollection(
					"patch_layer",
					authoritative.patch_layer,
					pendingOperations,
				) as ShowObject<"patch_layer">[])
			: previous.patchLayers,

		playbackPages: projectKinds.has("playback_page")
			? (projectCollection(
					"playback_page",
					authoritative.playback_page,
					pendingOperations,
				) as ShowObject<"playback_page">[])
			: previous.playbackPages,
		stageLayouts: projectKinds.has("stage_layout")
			? (projectCollection(
					"stage_layout",
					authoritative.stage_layout,
					pendingOperations,
				) as ShowObject<"stage_layout">[])
			: previous.stageLayouts,
		userLayouts: projectKinds.has("user_layout")
			? (projectCollection(
					"user_layout",
					authoritative.user_layout,
					pendingOperations,
				) as ShowObject<"user_layout">[])
			: previous.userLayouts,
		pendingObjectKeys: new Set(pendingKeys),
		...changes,
	};
}
