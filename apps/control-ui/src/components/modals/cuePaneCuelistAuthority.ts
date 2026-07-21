import { useMemo } from "react";
import type { PlaybackDefinition } from "../../api/types";
import { usePlaybackDefinitions } from "../../features/showObjects/ShowObjectsState";
import { useShowObjectView } from "../../features/showObjects/ShowObjectsView";

export interface CuePaneCuelistPlayback {
	number: number;
	name: string;
}

const NONE: readonly CuePaneCuelistPlayback[] = [];

function targetsCueList(
	definition: PlaybackDefinition,
): definition is PlaybackDefinition & {
	target: { type: "cue_list"; cue_list_id: string };
} {
	return definition.target.type === "cue_list";
}

/**
 * Hydrates Playback definitions only while Pane Settings is mounted for a Cues
 * pane. The picker keeps its existing contract: Cuelist Playbacks only, ordered
 * by Playback number.
 */
export function useCuePaneCuelistPlaybacks(
	enabled: boolean,
): readonly CuePaneCuelistPlayback[] {
	useShowObjectView("playback", enabled);
	const playbackObjects = usePlaybackDefinitions(enabled);
	return useMemo(() => {
		if (!enabled) return NONE;
		return playbackObjects
			.map((object) => object.body)
			.filter(targetsCueList)
			.map(({ number, name }) => ({ number, name }))
			.sort((left, right) => left.number - right.number);
	}, [enabled, playbackObjects]);
}
