import { useRef, useSyncExternalStore } from "react";
import type { StoredGroup, StoredPreset, VersionedObject } from "../../api/types";
import { ShowObjectsStore } from "../showObjects/store";

type GroupObject = VersionedObject<StoredGroup>;
type PresetObject = VersionedObject<StoredPreset>;

export function useShowObjectsState() {
	const showObjectsStore = useRef(new ShowObjectsStore()).current;
	const snapshot = useSyncExternalStore(
		showObjectsStore.subscribe,
		showObjectsStore.getSnapshot,
		showObjectsStore.getSnapshot,
	);
	return {
		showObjectsStore,
		groups: snapshot.groups as GroupObject[],
		presets: snapshot.presets as PresetObject[],
	};
}
