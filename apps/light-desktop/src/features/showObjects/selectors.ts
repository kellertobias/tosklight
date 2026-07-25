import type { ShowObject } from "./contracts";
import type { ShowObjectsSnapshot } from "./store";

export function selectPortableGroups(
	snapshot: ShowObjectsSnapshot,
): readonly ShowObject<"group">[] {
	return snapshot.groups;
}

export function selectPresets(
	snapshot: ShowObjectsSnapshot,
): readonly ShowObject<"preset">[] {
	return snapshot.presets;
}

export function selectCueLists(
	snapshot: ShowObjectsSnapshot,
): readonly ShowObject<"cue_list">[] {
	return snapshot.cueLists;
}

export function selectPlaybacks(
	snapshot: ShowObjectsSnapshot,
): readonly ShowObject<"playback">[] {
	return snapshot.playbacks;
}

export function selectPlaybackPages(
	snapshot: ShowObjectsSnapshot,
): readonly ShowObject<"playback_page">[] {
	return snapshot.playbackPages;
}
