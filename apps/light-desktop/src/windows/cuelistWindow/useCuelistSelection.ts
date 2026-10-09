import { useMemo } from "react";
import {
	useCueListRuntime,
	useDirectCueListProjectionMap,
} from "../../features/playbackRuntime/PlaybackRuntimeView";
import { legacyPlaybackRuntime } from "../../features/playbackRuntime/legacy";
import { cuelistPoolCatalog } from "../../features/playbackTopology/cuelistPoolCatalog";
import {
	useCueLists,
	usePlaybackDefinitions,
	usePlaybackPages,
} from "../../features/showObjects/ShowObjectsState";

export function useCuelistPool() {
	const lists = useCueLists();
	const playbacks = usePlaybackDefinitions();
	const pages = usePlaybackPages();
	return useMemo(
		() =>
			cuelistPoolCatalog(
				lists.map((object) => object.body),
				playbacks.map((object) => object.body),
				pages.length === 0,
			),
		[lists, playbacks, pages],
	);
}

export function useSelectedCuelist(
	selectedCuelist: number | null,
	enabled = true,
	fixedCueListId?: string,
	assignmentNumber?: number | null,
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
	const ownerNumber =
		assignmentNumber === null
			? undefined
			: (assignmentNumber ??
				(entry?.cueList.pool_number == null
					? entry?.assignment?.number
					: undefined));
	const assigned = useCueListRuntime(
		enabled && ownerNumber != null ? id : null,
		ownerNumber,
	);
	const direct = useDirectCueListProjectionMap(
		enabled && id ? [id] : [],
		enabled,
	);
	const projection = id ? direct.projections.get(id) : undefined;
	const active =
		ownerNumber != null ? assigned : legacyPlaybackRuntime(projection);
	return {
		pool,
		selectedPlaybackDefinition: entry,
		selectedCueObject,
		cueList: selectedCueObject?.body,
		active,
	};
}
