import { useMemo } from "react";
import type { ShowObjectKind } from "../showObjects/contracts";
import {
	useCueLists,
	usePlaybackDefinitions,
	usePlaybackPages,
	useShowObjectCollectionsReady,
	useShowObjectsStatus,
} from "../showObjects/ShowObjectsState";
import { useShowObjectKindsView } from "../showObjects/ShowObjectsView";
import type { PlaybackPagesView, PlaybackTopologyView } from "./contracts";

const TOPOLOGY_KINDS = [
	"cue_list",
	"playback",
	"playback_page",
] as const satisfies readonly ShowObjectKind[];
const PAGE_KINDS = [
	"playback_page",
] as const satisfies readonly ShowObjectKind[];

/** Hydrates and subscribes to the complete portable topology only while visible. */
export function usePlaybackTopologyView(enabled = true): PlaybackTopologyView {
	useShowObjectKindsView(TOPOLOGY_KINDS, enabled);
	const collectionsReady = useShowObjectCollectionsReady(
		TOPOLOGY_KINDS,
		enabled,
	);
	const authorityReady = enabled && collectionsReady;
	const { error } = useShowObjectsStatus(enabled);
	const cueLists = useCueLists(enabled);
	const playbacks = usePlaybackDefinitions(enabled);
	const pages = usePlaybackPages(enabled);
	return useMemo(
		() => ({
			ready: authorityReady,
			error: enabled ? error : null,
			cueLists: authorityReady ? cueLists : [],
			playbacks: authorityReady ? playbacks : [],
			pages: authorityReady ? pages : [],
		}),
		[authorityReady, cueLists, enabled, error, pages, playbacks],
	);
}

/** Hydrates only portable Page definitions needed by always-visible page chrome. */
export function usePlaybackPagesView(enabled = true): PlaybackPagesView {
	useShowObjectKindsView(PAGE_KINDS, enabled);
	const ready = useShowObjectCollectionsReady(PAGE_KINDS, enabled);
	const authorityReady = enabled && ready;
	const { error } = useShowObjectsStatus(enabled);
	const pages = usePlaybackPages(enabled);
	return useMemo(
		() => ({
			ready: authorityReady,
			error: enabled ? error : null,
			pages: authorityReady ? pages : [],
		}),
		[authorityReady, enabled, error, pages],
	);
}
