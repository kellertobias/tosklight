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
