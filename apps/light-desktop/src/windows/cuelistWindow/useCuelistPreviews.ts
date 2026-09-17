import { useMemo } from "react";
import type { Cue } from "../../api/types";
import {
	type CueMediaPreview,
	useCueMediaPreviews,
} from "../../features/cueThumbnails/useCueMediaPreviews";
import { useCueThumbnails } from "./useCueThumbnails";

/**
 * Every picture a Cuelist's Preview column shows.
 *
 * Media-only Cues are pictured by their Media Server. The Stage path waits until the desk has said
 * which Cues those are, so it never draws or stores a Stage picture in their place. `override`
 * replaces the Stage pictures entirely (a caller that already holds them).
 */
export function useCuelistPreviews(
	cues: Cue[],
	active: boolean,
	override?: Record<number, string>,
) {
	const cueIds = useMemo(
		() => cues.flatMap((cue) => (cue.id ? [cue.id] : [])),
		[cues],
	);
	const media = useCueMediaPreviews(cueIds, active && !override, cues);
	const generated = useCueThumbnails(cues, active && media.ready, {
		exclude: media.mediaCueIds,
	});
	const thumbnails = override ?? generated;
	const mediaPreviews = useMemo(() => {
		const rows: Record<number, CueMediaPreview> = {};
		cues.forEach((cue, index) => {
			const preview = cue.id ? media.previews.get(cue.id) : undefined;
			if (preview) rows[index] = preview;
		});
		return rows;
	}, [cues, media.previews]);
	/** The picture a Cue row opens larger, and whether it is a layer picture. */
	const pictureAt = (index: number | null) => {
		if (index === null) return undefined;
		const mediaPreview = mediaPreviews[index];
		if (!mediaPreview) {
			const src = thumbnails[index];
			return src ? { src, scope: null } : undefined;
		}
		return "src" in mediaPreview
			? { src: mediaPreview.src, scope: mediaPreview.entry.scope }
			: undefined;
	};
	return { thumbnails, mediaPreviews, retry: media.retry, pictureAt };
}
