import { useCallback, useMemo } from "react";
import type { PlaybackDefinition, PlaybackPage } from "../../../api/types";
import type { PlaybackInteractionIdentity } from "../../../features/controlSurfaceInteraction/contracts";
import {
	usePlaybackDeskView,
	usePlaybackRuntimeStatus,
} from "../../../features/playbackRuntime/PlaybackRuntimeView";
import type { ShowObjectKind } from "../../../features/showObjects/contracts";
import {
	usePlaybackDefinitions,
	usePlaybackPages,
	useShowObjectCollectionsReady,
} from "../../../features/showObjects/ShowObjectsState";
import { useShowObjectKindsView } from "../../../features/showObjects/ShowObjectsView";

/** Keyboard shortcuts address Pages and their mapped Playbacks, nothing else. */
const SHORTCUT_KINDS = [
	"playback_page",
	"playback",
] as const satisfies readonly ShowObjectKind[];

export interface PlaybackShortcutAuthority {
	/** True only once Page definitions, Playback definitions and the desk are authoritative. */
	ready: boolean;
	/** The exact desk Page; null while the desk projection is still loading. */
	activePage: number | null;
	pages: readonly PlaybackPage[];
	/** Resolves slot 1-8 on the authoritative current Page. */
	slotPlayback: (slot: number) => PlaybackDefinition | null;
	playbackIdentity: (slot: number) => PlaybackInteractionIdentity | null;
}

const NO_PAGES: readonly PlaybackPage[] = [];

const DORMANT: PlaybackShortcutAuthority = {
	ready: false,
	activePage: null,
	pages: NO_PAGES,
	slotPlayback: () => null,
	playbackIdentity: () => null,
};

/**
 * Hydrates only the Page and Playback definitions the keyboard needs, and only
 * while the keyboard actually owns the keys. Dormant shortcuts open no
 * subscription at all.
 */
export function usePlaybackShortcutAuthority(
	enabled: boolean,
): PlaybackShortcutAuthority {
	useShowObjectKindsView(SHORTCUT_KINDS, enabled);
	const definitionsReady = useShowObjectCollectionsReady(
		SHORTCUT_KINDS,
		enabled,
	);
	const pageObjects = usePlaybackPages(enabled);
	const playbackObjects = usePlaybackDefinitions(enabled);
	const desk = usePlaybackDeskView(enabled);
	const runtimeStatus = usePlaybackRuntimeStatus(enabled);
	const runtimeReady = enabled && runtimeStatus.status === "ready";
	const activePage = runtimeReady ? (desk?.active_page ?? null) : null;
	const ready = definitionsReady && runtimeReady && activePage != null;
	const pages = useMemo(
		() => (ready ? pageObjects.map((page) => page.body) : NO_PAGES),
		[pageObjects, ready],
	);
	const playbacks = useMemo(
		() => (ready ? playbackObjects.map((playback) => playback.body) : []),
		[playbackObjects, ready],
	);
	const slotPlayback = useCallback(
		(slot: number) => {
			if (!ready) return null;
			const page = pages.find((candidate) => candidate.number === activePage);
			const playbackNumber = page?.slots[String(slot)];
			if (playbackNumber == null) return null;
			const match = playbacks.find(
				(candidate) => candidate.number === playbackNumber,
			);
			return match ?? null;
		},
		[activePage, pages, playbacks, ready],
	);
	const playbackIdentity = useCallback(
		(slot: number): PlaybackInteractionIdentity | null => {
			if (!ready || activePage == null) return null;
			const page = pageObjects.find(
				(candidate) => candidate.body.number === activePage,
			);
			const playbackNumber = page?.body.slots[String(slot)];
			const playback = playbackObjects.find(
				(candidate) => candidate.body.number === playbackNumber,
			);
			return {
				addressing: "current_page",
				pageNumber: activePage,
				slot,
				pageObjectId: page?.id ?? null,
				pageObjectRevision: page?.revision ?? 0,
				playbackObjectId: playback?.id ?? null,
				playbackObjectRevision: playback?.revision ?? 0,
			};
		},
		[activePage, pageObjects, playbackObjects, ready],
	);
	return useMemo(
		() =>
			enabled
				? { ready, activePage, pages, slotPlayback, playbackIdentity }
				: DORMANT,
		[activePage, enabled, pages, playbackIdentity, ready, slotPlayback],
	);
}
