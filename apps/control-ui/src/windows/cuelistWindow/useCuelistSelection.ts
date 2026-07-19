import { useMemo } from "react";
import { useServer } from "../../api/ServerContext";

export function useCuelistPool() {
	const server = useServer();
	return useMemo(
		() =>
			(server.playbacks?.pool ?? [])
				.filter((definition) => definition.target.type === "cue_list")
				.sort((left, right) => left.number - right.number),
		[server.playbacks?.pool],
	);
}

export function useSelectedCuelist(selectedCuelist: number | null) {
	const server = useServer();
	const pool = useCuelistPool();
	const selectedPlaybackDefinition = server.playbacks?.pool.find(
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
		pool.length === 0 && selectedCuelist === 1
			? server.cueObjects?.[0]
			: undefined;
	const selectedCueObject = selectedCueListId
		? server.cueObjects?.find((candidate) => candidate.id === selectedCueListId)
		: legacyFirstCueObject;
	const cueList =
		selectedCueObject?.body ??
		(selectedCueListId
			? server.playbacks?.cue_lists.find(
					(candidate) => candidate.id === selectedCueListId,
				)
			: pool.length === 0 && selectedCuelist === 1
				? server.playbacks?.cue_lists[0]
				: undefined);
	const active =
		cueList &&
		server.playbacks?.active.find((item) => item.cue_list_id === cueList.id);
	return {
		pool,
		selectedPlaybackDefinition,
		selectedCueObject,
		cueList,
		active,
	};
}
