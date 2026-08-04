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
	/** Resolves the first physical button, including a claimed wider right column. */
	slotControl?: (slot: number) => {
		playback: PlaybackDefinition;
		action: PlaybackDefinition["buttons"][number];
		button: number;
	} | null;
	playbackIdentity: (slot: number) => PlaybackInteractionIdentity | null;
}

const NO_PAGES: readonly PlaybackPage[] = [];

const DORMANT: PlaybackShortcutAuthority = {
	ready: false,
	activePage: null,
	pages: NO_PAGES,
	slotPlayback: () => null,
	slotControl: () => null,
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
			let resolvedSlot = slot;
			let playbackNumber = page?.body.slots[String(resolvedSlot)];
			if (playbackNumber == null && slot > 1) {
				const leftNumber = page?.body.slots[String(slot - 1)];
				const left = playbackObjects.find(
					(candidate) => candidate.body.number === leftNumber,
				);
				if (left?.body.footprint?.type === "wider") {
					resolvedSlot = slot - 1;
					playbackNumber = leftNumber;
				}
			}
			const playback = playbackObjects.find(
				(candidate) => candidate.body.number === playbackNumber,
			);
			return {
				addressing: "current_page",
				pageNumber: activePage,
				slot: resolvedSlot,
				pageObjectId: page?.id ?? null,
				pageObjectRevision: page?.revision ?? 0,
				playbackObjectId: playback?.id ?? null,
				playbackObjectRevision: playback?.revision ?? 0,
			};
		},
		[activePage, pageObjects, playbackObjects, ready],
	);
	const slotControl = useCallback(
		(slot: number) => {
			const direct = slotPlayback(slot);
			if (direct)
				return { playback: direct, action: direct.buttons[0], button: 1 };
			if (!ready || slot <= 1) return null;
			const left = slotPlayback(slot - 1);
			if (left?.footprint?.type !== "wider") return null;
			return {
				playback: left,
				action: left.footprint.right_buttons[0],
				button: 4,
			};
		},
		[ready, slotPlayback],
	);
	return useMemo(
		() =>
			enabled
				? {
						ready,
						activePage,
						pages,
						slotPlayback,
						slotControl,
						playbackIdentity,
					}
				: DORMANT,
		[
			activePage,
			enabled,
			pages,
			playbackIdentity,
			ready,
			slotControl,
			slotPlayback,
		],
	);
}
