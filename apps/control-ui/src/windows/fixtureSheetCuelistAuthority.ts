import { useMemo } from "react";
import type { CueList } from "../api/types";
import type { ShowObjectKind } from "../features/showObjects/contracts";
import {
	useCueLists,
	useShowObjectCollectionsReady,
} from "../features/showObjects/ShowObjectsState";
import { useShowObjectView } from "../features/showObjects/ShowObjectsView";

/** The Fixture Sheet Cuelist filter reads Cuelist definitions and nothing else. */
const CUELIST_KINDS = ["cue_list"] as const satisfies readonly ShowObjectKind[];

export interface FixtureSheetCuelistOption {
	id: string;
	name: string;
}

export interface FixtureSheetCuelistAuthority {
	/** True only once the scoped Cuelist collection is authoritative for this show. */
	ready: boolean;
	/** Picker options; empty while the sheet is dormant or the collection is loading. */
	cueLists: readonly FixtureSheetCuelistOption[];
	/** The saved choice resolved against authority; "" means "All fixtures". */
	selectedCueListId: string;
	/** Exact selected Cuelist used by the fixture filter; null means no filter. */
	selectedCueList: CueList | null;
}

const NO_OPTIONS: readonly FixtureSheetCuelistOption[] = [];

const DORMANT: FixtureSheetCuelistAuthority = {
	ready: false,
	cueLists: NO_OPTIONS,
	selectedCueListId: "",
	selectedCueList: null,
};

/**
 * Hydrates and subscribes to Cuelist definitions only while a Fixture Sheet that
 * can filter by Cuelist is actually visible. A dormant or compact sheet opens no
 * snapshot and no socket, and a replaced scope never exposes cached Cuelists:
 * the saved choice stays untouched and simply resolves to "All fixtures" until
 * the replacement collection is authoritative again.
 */
export function useFixtureSheetCuelistAuthority({
	enabled,
	savedCueListId,
}: {
	enabled: boolean;
	savedCueListId: string;
}): FixtureSheetCuelistAuthority {
	useShowObjectView("cue_list", enabled);
	const collectionReady = useShowObjectCollectionsReady(CUELIST_KINDS, enabled);
	const cueListObjects = useCueLists(enabled);
	const ready = enabled && collectionReady;
	return useMemo(() => {
		if (!ready) return DORMANT;
		const bodies = cueListObjects.map((object) => object.body);
		const selected =
			bodies.find((body) => body.id === savedCueListId) ?? null;
		return {
			ready,
			cueLists: bodies.map((body) => ({ id: body.id, name: body.name })),
			selectedCueListId: selected?.id ?? "",
			selectedCueList: selected,
		};
	}, [cueListObjects, ready, savedCueListId]);
}
