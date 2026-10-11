import type { PlaybackArtworkModel } from "@tosklight/ui/playback";
import { type PropsWithChildren, useMemo, useRef } from "react";
import type { Cue, CueList, PlaybackDefinition } from "../../../api/types";
import { cueMediaPreviewNotice } from "../../../features/cueThumbnails/CueMediaPreviewView";
import { useCueMediaPreviews } from "../../../features/cueThumbnails/useCueMediaPreviews";
import { useCueThumbnails } from "../../../windows/cuelistWindow/useCueThumbnails";
import {
	automaticPlaybackCue,
	PlaybackArtworkContext,
} from "./usePlaybackArtwork";

type Candidate = { playback: PlaybackDefinition | null; cue: CueList | null };

/** Runtime/fader ticks do not invalidate an unchanged set of authored Cue objects. */
function useVisibleCues(candidates: readonly Candidate[]) {
	const previous = useRef<Cue[]>([]);
	const seen = new Set<string | Cue>();
	const next: Cue[] = [];
	for (const candidate of candidates) {
		const cue = automaticPlaybackCue(candidate.playback, candidate.cue);
		if (!cue || seen.has(cue.id ?? cue)) continue;
		seen.add(cue.id ?? cue);
		next.push(cue);
	}
	if (
		next.length !== previous.current.length ||
		next.some((cue, index) => cue !== previous.current[index])
	)
		previous.current = next;
	return previous.current;
}

/** One scoped authority subscription and preview batch for this rendered surface/page. */
export function PlaybackArtworkProvider({
	candidates,
	children,
}: PropsWithChildren<{ candidates: readonly Candidate[] }>) {
	const cues = useVisibleCues(candidates);
	const active = cues.length > 0;
	const ids = useMemo(
		() => cues.flatMap((cue) => (cue.id ? [cue.id] : [])),
		[cues],
	);
	const media = useCueMediaPreviews(ids, active, cues);
	const thumbnails = useCueThumbnails(cues, active && media.ready, {
		exclude: media.mediaCueIds,
		independent: true,
	});
	const artwork = useMemo(() => {
		const result = new Map<Cue, PlaybackArtworkModel>();
		cues.forEach((cue, index) => {
			const preview = cue.id ? media.previews.get(cue.id) : undefined;
			if (preview)
				result.set(cue, {
					image: "src" in preview ? preview.src : undefined,
					imageSource: "src" in preview ? "cue-preview" : undefined,
					transparent: preview.entry.scope === "layer" && "src" in preview,
					notice: cueMediaPreviewNotice(preview) ?? undefined,
				});
			else if (!media.mediaCueIds.has(cue.id ?? "") && thumbnails[index])
				result.set(cue, {
					image: thumbnails[index],
					imageSource: "cue-preview",
				});
		});
		return result;
	}, [cues, media.previews, media.mediaCueIds, thumbnails]);
	return (
		<PlaybackArtworkContext.Provider value={artwork}>
			{children}
		</PlaybackArtworkContext.Provider>
	);
}
