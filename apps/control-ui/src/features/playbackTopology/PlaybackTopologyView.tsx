import { useMemo } from "react";
import {
	useCueLists,
	usePlaybackDefinitions,
	usePlaybackPages,
	useShowObjectCollectionsReady,
	useShowObjectsStatus,
} from "../showObjects/ShowObjectsState";
import { useShowObjectKindsView } from "../showObjects/ShowObjectsView";
import type { ShowObjectKind } from "../showObjects/contracts";
import type { PlaybackTopologyView } from "./contracts";

const TOPOLOGY_KINDS = [
	"cue_list",
	"playback",
	"playback_page",
] as const satisfies readonly ShowObjectKind[];

/** Hydrates and subscribes to the complete portable topology only while visible. */
export function usePlaybackTopologyView(enabled = true): PlaybackTopologyView {
	useShowObjectKindsView(TOPOLOGY_KINDS, enabled);
	const collectionsReady = useShowObjectCollectionsReady(TOPOLOGY_KINDS, enabled);
	const { error } = useShowObjectsStatus(enabled);
	const cueLists = useCueLists(enabled);
	const playbacks = usePlaybackDefinitions(enabled);
	const pages = usePlaybackPages(enabled);
	return useMemo(
		() => ({
			ready: enabled && collectionsReady,
			error: enabled ? error : null,
			cueLists: enabled ? cueLists : [],
			playbacks: enabled ? playbacks : [],
			pages: enabled ? pages : [],
		}),
		[collectionsReady, cueLists, enabled, error, pages, playbacks],
	);
}
