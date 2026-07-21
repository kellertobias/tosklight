import { useCallback, useSyncExternalStore } from "react";
import type { ShowObject } from "../../../features/showObjects/contracts";
import { useShowObjectsStore } from "../../../features/showObjects/ShowObjectsState";
import { useShowObjectView } from "../../../features/showObjects/ShowObjectsView";

type GroupObject = ShowObject<"group">;

const NO_SUBSCRIPTION = () => () => undefined;
const NO_GROUP_ATTRIBUTES: readonly string[] = [];

/**
 * Hydrates the portable Group collection only for a visible Group parameter target.
 * A retained object is hidden until that collection is authoritative, so this hook
 * never borrows stale Group programming from the broad Playback snapshot.
 */
export function useSelectedPortableGroup(
	groupId: string | null,
	active = true,
): GroupObject | null | undefined {
	const enabled = active && groupId !== null;
	useShowObjectView("group", enabled);
	const store = useShowObjectsStore();
	const getSelection = useCallback(() => {
		if (!enabled || groupId === null) return undefined;
		const snapshot = store.getSnapshot();
		if (!snapshot.readyCollections.has("group")) return undefined;
		return snapshot.groups.find((group) => group.id === groupId) ?? null;
	}, [enabled, groupId, store]);
	return useSyncExternalStore(
		enabled ? store.subscribe : NO_SUBSCRIPTION,
		getSelection,
		getSelection,
	);
}

/** Intensity is intrinsic to a selected Group; other attributes require its portable body. */
export function selectedGroupSupportedAttributes(
	groupId: string | null,
	group: GroupObject | null | undefined,
): readonly string[] {
	if (groupId === null) return NO_GROUP_ATTRIBUTES;
	const programmed =
		group?.id === groupId ? Object.keys(group.body.programming ?? {}) : [];
	return ["intensity", ...programmed.filter((name) => name !== "intensity")];
}
