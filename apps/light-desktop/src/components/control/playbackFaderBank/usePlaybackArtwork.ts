import type { PlaybackArtworkModel } from "@tosklight/ui/playback";
import { createContext, useContext } from "react";
import type { Cue, CueList, PlaybackDefinition } from "../../../api/types";

export const PlaybackArtworkContext = createContext<
	ReadonlyMap<Cue, PlaybackArtworkModel>
>(new Map());

export function automaticPlaybackCue(
	playback: PlaybackDefinition | null,
	cuelist: CueList | null,
) {
	return playback?.target.type === "cue_list" &&
		!playback.presentation_icon?.trim() &&
		!playback.presentation_image?.trim() &&
		cuelist?.cues.length === 1
		? cuelist.cues[0]
		: undefined;
}

/** Cards read one visible-bank batch; they never fetch or render previews themselves. */
export function usePlaybackArtwork(
	playback: PlaybackDefinition | null,
	cuelist: CueList | null,
): PlaybackArtworkModel | undefined {
	const previews = useContext(PlaybackArtworkContext);
	const icon = playback?.presentation_icon?.trim();
	const image = playback?.presentation_image?.trim();
	if (icon) return { icon };
	if (image) return { image, imageSource: "configured" };
	const cue = automaticPlaybackCue(playback, cuelist);
	return cue ? previews.get(cue) : undefined;
}
