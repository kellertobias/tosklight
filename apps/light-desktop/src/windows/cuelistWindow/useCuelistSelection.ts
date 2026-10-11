import { useMemo } from "react";
import {
	useCueListRuntime,
	useDirectCueListProjectionMap,
} from "../../features/playbackRuntime/PlaybackRuntimeView";
import { playbackControlRuntime } from "../../features/playbackRuntime/controlRuntime";
import { cuelistPoolCatalog } from "../../features/playbackTopology/cuelistPoolCatalog";
import {
	useCueLists,
	usePlaybackPages,
} from "../../features/showObjects/ShowObjectsState";

export function useCuelistPool() {
	const lists = useCueLists();
	const pages = usePlaybackPages();
	return useMemo(
		() =>
			cuelistPoolCatalog(
				lists.map((object) => object.body),
				pages.flatMap(({body}) => Object.entries(body.assignments).map(([position, definition]) => ({...definition, address: {page: body.number, position: Number(position)}}))),
				pages.length === 0,
			),
		[lists, pages],
	);
}

export function useSelectedCuelist(
	selectedCuelist: number | null,
	enabled = true,
	fixedCueListId?: string,
) {
	const pool = useCuelistPool();
	const cueLists = useCueLists();
	const entry =
		fixedCueListId !== undefined
			? pool.find((entry) => entry.cueList.id === fixedCueListId)
			: pool.find((entry) => entry.number === selectedCuelist);
	const id = fixedCueListId ?? entry?.cueList.id ?? null;
	const selectedCueObject = id
		? cueLists.find((object) => object.body.id === id)
		: undefined;
	const active = useCueListRuntime(enabled ? id : null);
	return {
		pool,
		selectedPlaybackDefinition: entry,
		selectedCueObject,
		cueList: selectedCueObject?.body,
		active,
	};
}
