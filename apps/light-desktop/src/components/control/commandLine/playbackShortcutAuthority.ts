import type { AssignedPlayback } from "../playbackFaderBank/types";
import { useCallback, useMemo } from "react";
import type { PlaybackDefinition, PlaybackPage } from "../../../api/types";
import type { PlaybackInteractionIdentity } from "../../../features/controlSurfaceInteraction/contracts";
import {
	usePlaybackDeskView,
	usePlaybackRuntimeStatus,
} from "../../../features/playbackRuntime/PlaybackRuntimeView";
import type { ShowObjectKind } from "../../../features/showObjects/contracts";
import {
	usePlaybackPages,
	useShowObjectCollectionsReady,
} from "../../../features/showObjects/ShowObjectsState";
import { useShowObjectKindsView } from "../../../features/showObjects/ShowObjectsView";

/** Keyboard shortcuts address Pages and their mapped Playbacks, nothing else. */
const SHORTCUT_KINDS = [
	"playback_page",
] as const satisfies readonly ShowObjectKind[];

export interface PlaybackShortcutAuthority {
	/** True only once Page definitions, Playback definitions and the desk are authoritative. */
	ready: boolean;
	/** The exact desk Page; null while the desk projection is still loading. */
	activePage: number | null;
	pages: readonly PlaybackPage[];
	/** Resolves slot 1-8 on the authoritative current Page. */
	surfaceId: string;
	slotPlayback: (slot: number) => AssignedPlayback | null;
	/** Resolves the first physical button, including a claimed wider right column. */
	slotControl?: (slot: number) => {
		playback: AssignedPlayback;
		action: PlaybackDefinition["buttons"][number];
		button: number;
	} | null;
	playbackIdentity: (slot: number) => PlaybackInteractionIdentity | null;
}

const NO_PAGES: readonly PlaybackPage[] = [];

const DORMANT: PlaybackShortcutAuthority = {
	surfaceId: "main",
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
	const desk = usePlaybackDeskView(enabled);
	const runtimeStatus = usePlaybackRuntimeStatus(enabled);
	const runtimeReady = enabled && runtimeStatus.status === "ready";
	const activePage = runtimeReady ? (desk?.command_page ?? null) : null;
	const ready = definitionsReady && runtimeReady && activePage != null;
	const pages = useMemo(
		() => (ready ? pageObjects.map((page) => page.body) : NO_PAGES),
		[pageObjects, ready],
	);
	const surfaceId = desk?.selected_surface_id ?? "main";
	const slotPlayback = useCallback((position: number): AssignedPlayback | null => {
		if (!ready || activePage == null) return null;
		const definition = pages.find(candidate => candidate.number === activePage)?.assignments[String(position)];
		return definition ? {...definition, address: {page: activePage, position}} : null;
	}, [activePage, pages, ready]);
	const playbackIdentity = useCallback(
		(slot: number): PlaybackInteractionIdentity | null => {
			if (!ready || activePage == null) return null;
			const page = pageObjects.find(
				(candidate) => candidate.body.number === activePage,
			);
			let resolvedSlot = slot;
			if (!page?.body.assignments[String(slot)] && slot > 1 && page?.body.assignments[String(slot - 1)]?.footprint?.type === "wider") resolvedSlot = slot - 1;
			return {
				addressing: "current_page",
				pageNumber: activePage,
				slot: resolvedSlot,
				pageObjectId: page?.id ?? null,
				pageObjectRevision: page?.revision ?? 0,
			};
		},
		[activePage, pageObjects, ready],
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
						surfaceId,
						ready,
			activePage,
						pages,
						slotPlayback,
						slotControl,
						playbackIdentity,
					}
				: DORMANT,
		[
			surfaceId,
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
