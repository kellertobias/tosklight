import type { ShowObject } from "./contracts";
import type { ShowObjectsSnapshot } from "./store";

export function selectDynamics(
	snapshot: ShowObjectsSnapshot,
): readonly ShowObject<"dynamic">[] {
	return snapshot.dynamics;
}

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


export function selectPlaybackPages(
	snapshot: ShowObjectsSnapshot,
): readonly ShowObject<"playback_page">[] {
	return snapshot.playbackPages;
}

export function selectMacros(
	snapshot: ShowObjectsSnapshot,
): readonly ShowObject<"macro">[] {
	return snapshot.macros;
}
export function selectTimecodes(
	snapshot: ShowObjectsSnapshot,
): readonly ShowObject<"timecode">[] {
	return snapshot.timecodes;
}
