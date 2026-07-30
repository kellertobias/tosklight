import { useMemo } from "react";
import { useCueListRuntime } from "../../features/playbackRuntime/PlaybackRuntimeView";
import {
	useCueLists,
	usePlaybackDefinitions,
} from "../../features/showObjects/ShowObjectsState";

export function useCuelistPool() {
	const playbacks = usePlaybackDefinitions();
	return useMemo(
		() =>
			playbacks
				.map((object) => object.body)
				.filter((definition) => definition.target.type === "cue_list")
				.sort((left, right) => left.number - right.number),
		[playbacks],
	);
}

export function useSelectedCuelist(
	selectedCuelist: number | null,
	enabled = true,
	fixedCueListId?: string,
) {
	const pool = useCuelistPool();
	const cueLists = useCueLists();
	const hasFixedCueListId = fixedCueListId !== undefined;
	const selectedPlaybackDefinition = hasFixedCueListId
		? pool.find(
				(definition) =>
					definition.target.type === "cue_list" &&
					definition.target.cue_list_id === fixedCueListId,
			)
		: pool.find((definition) => definition.number === selectedCuelist);
	const selectedDefinition =
		selectedPlaybackDefinition?.target.type === "cue_list"
			? selectedPlaybackDefinition
			: undefined;
	const selectedCueListId = hasFixedCueListId
		? fixedCueListId
		: selectedDefinition?.target.type === "cue_list"
			? selectedDefinition.target.cue_list_id
			: null;
	const legacyFirstCueObject =
		!hasFixedCueListId && pool.length === 0 && selectedCuelist === 1
			? cueLists[0]
			: undefined;
	const selectedCueObject = selectedCueListId
		? cueLists.find((candidate) => candidate.body.id === selectedCueListId)
		: legacyFirstCueObject;
	const cueList = selectedCueObject?.body;
	// The exact Cuelist runtime is the only live authority; an absent projection
	// means "not running", never a reason to read a broad Playback snapshot.
	const active = useCueListRuntime(
		enabled ? selectedCueListId : null,
		selectedDefinition?.number,
	);
	return {
		pool,
		selectedPlaybackDefinition,
		selectedCueObject,
		cueList,
		active,
	};
}
