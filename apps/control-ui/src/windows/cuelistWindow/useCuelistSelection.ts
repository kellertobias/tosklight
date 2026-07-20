import { useMemo } from "react";
import { useServer } from "../../api/ServerContext";
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
) {
	const server = useServer();
	const pool = useCuelistPool();
	const cueLists = useCueLists();
	const selectedPlaybackDefinition = pool.find(
		(definition) => definition.number === selectedCuelist,
	);
	const selectedDefinition =
		selectedPlaybackDefinition?.target.type === "cue_list"
			? selectedPlaybackDefinition
			: undefined;
	const selectedCueListId =
		selectedDefinition?.target.type === "cue_list"
			? selectedDefinition.target.cue_list_id
			: null;
	const legacyFirstCueObject =
		pool.length === 0 && selectedCuelist === 1 ? cueLists[0] : undefined;
	const selectedCueObject = selectedCueListId
		? cueLists.find((candidate) => candidate.id === selectedCueListId)
		: legacyFirstCueObject;
	const cueList = selectedCueObject?.body;
	const liveActive = useCueListRuntime(
		enabled ? selectedCueListId : null,
		selectedDefinition?.number,
	);
	return {
		pool,
		selectedPlaybackDefinition,
		selectedCueObject,
		cueList,
		active:
			liveActive ??
			(cueList
				? server.playbacks?.active.find(
						(item) => item.cue_list_id === cueList.id,
					)
				: undefined),
	};
}
