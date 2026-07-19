import { useMemo, useRef } from "react";
import type { PlaybackSnapshot } from "../../api/types";
import { usePortableGroups } from "../showObjects/ShowObjectsState";
import { ShowObjectsStore } from "../showObjects/store";
import { projectRuntimeGroupMasters } from "./groupRuntimeProjection";

export function useShowObjectsState() {
	const showObjectsStore = useRef(new ShowObjectsStore()).current;
	return { showObjectsStore };
}

export function useGroups(playbacks: PlaybackSnapshot | null) {
	const portableGroups = usePortableGroups();
	return useMemo(
		() =>
			projectRuntimeGroupMasters(
				portableGroups,
				playbacks?.authoritative_controls?.groups,
			),
		[playbacks?.authoritative_controls?.groups, portableGroups],
	);
}
