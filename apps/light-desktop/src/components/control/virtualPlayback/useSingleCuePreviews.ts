import { useMemo } from "react";
import type { Cue, CueList, PlaybackPage } from "../../../api/types";
import { cueMediaPreviewNotice } from "../../../features/cueThumbnails/CueMediaPreviewView";
import { useCueMediaPreviews } from "../../../features/cueThumbnails/useCueMediaPreviews";
import { useCueThumbnails } from "../../../windows/cuelistWindow/useCueThumbnails";

/** The automatic image of one Virtual Playback, taken from its only Cue's preview. */
export interface SingleCuePreview {
	cueId: string;
	src?: string;
	/** A layer preview keeps transparency. */
	transparent: boolean;
	/** Why there is no (or only an empty) picture right now. */
	notice?: string;
}

function hasConfiguredPresentation(playback: {
	presentation_icon?: string | null;
	presentation_image?: string | null;
}) {
	return Boolean(
		playback.presentation_icon?.trim() || playback.presentation_image?.trim(),
	);
}

/**
 * The only Cue of every Cuelist on this page that shows its Cue preview by default.
 *
 * A Virtual Playback qualifies while its Cuelist has exactly one Cue and the operator has chosen
 * neither an icon nor an image; the operator's choice always wins. Adding a second Cue, removing
 * the Cue, or choosing an icon or image removes the automatic default at once, and editing the Cue
 * changes it, because both follow the pushed Cuelist content.
 */
export function singleCueCandidates(
	page: PlaybackPage | undefined,
	cueLists: ReadonlyMap<string, CueList>,
): Map<number, Cue> {
	const candidates = new Map<number, Cue>();
	for (const [key, playback] of Object.entries(page?.virtual_playbacks ?? {})) {
		if (!playback || playback.target.type !== "cue_list") continue;
		if (hasConfiguredPresentation(playback)) continue;
		const cues = cueLists.get(playback.target.cue_list_id)?.cues ?? [];
		if (cues.length !== 1 || !cues[0].id) continue;
		candidates.set(Number(key), cues[0]);
	}
	return candidates;
}

export function useSingleCuePreviews(
	page: PlaybackPage | undefined,
	cueLists: ReadonlyMap<string, CueList>,
	active = true,
): ReadonlyMap<number, SingleCuePreview> {
	const candidates = useMemo(
		() => singleCueCandidates(page, cueLists),
		[page, cueLists],
	);
	const cues = useMemo(() => {
		const unique = new Map<string, Cue>();
		for (const cue of candidates.values())
			if (cue.id) unique.set(cue.id, cue);
		return [...unique.values()];
	}, [candidates]);
	const cueIds = useMemo(
		() => cues.flatMap((cue) => (cue.id ? [cue.id] : [])),
		[cues],
	);
	const media = useCueMediaPreviews(cueIds, active && cues.length > 0, cues);
	const stage = useCueThumbnails(
		cues,
		active && cues.length > 0 && media.ready,
		{ exclude: media.mediaCueIds, independent: true },
	);
	return useMemo(() => {
		const stageById = new Map<string, string>();
		cues.forEach((cue, index) => {
			if (cue.id && stage[index]) stageById.set(cue.id, stage[index]);
		});
		const previews = new Map<number, SingleCuePreview>();
		for (const [number, cue] of candidates) {
			const cueId = cue.id as string;
			const mediaPreview = media.previews.get(cueId);
			if (mediaPreview) {
				previews.set(number, {
					cueId,
					src: "src" in mediaPreview ? mediaPreview.src : undefined,
					transparent: mediaPreview.entry.scope === "layer",
					notice: cueMediaPreviewNotice(mediaPreview) ?? undefined,
				});
				continue;
			}
			if (media.mediaCueIds.has(cueId)) {
				// Known to be a media Cue, picture not requested yet: never a Stage stand-in.
				previews.set(number, {
					cueId,
					transparent: false,
					notice: "Loading media preview",
				});
				continue;
			}
			const src = stageById.get(cueId);
			if (src) previews.set(number, { cueId, src, transparent: false });
		}
		return previews;
	}, [candidates, cues, media.mediaCueIds, media.previews, stage]);
}
