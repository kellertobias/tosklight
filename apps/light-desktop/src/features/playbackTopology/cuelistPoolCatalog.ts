import type { AssignedPlayback } from "../../components/control/playbackFaderBank/types";
import type { CueList, PlaybackDefinition } from "../../api/types";

export interface CuelistPoolEntry {
	number: number;
	canonicalNumber: number;
	legacyAlias: boolean;
	cueList: CueList;
	// Presentation and assignment-owned runtime may use this definition. Its number is
	// never the operator Cuelist address after explicit metadata has been authored.
	assignment?: AssignedPlayback;
}

export function cuelistPoolCatalog(
	lists: readonly CueList[],
	playbacks: readonly AssignedPlayback[],
	pagesAbsent = false,
): CuelistPoolEntry[] {
	const owners = new Map<number, CueList>();
	const canonical = new Map<string, number>();
	const ids = new Set<string>();
	const claim = (number: number, list: CueList) => {
		if (!Number.isInteger(number) || number < 1 || number > 1000)
			throw new Error("Cuelist address must be within 1-1000");
		const previous = owners.get(number);
		if (previous && previous.id !== list.id)
			throw new Error(`Cuelist address ${number} is ambiguous`);
		owners.set(number, list);
	};
	for (const list of lists) {
		if (ids.has(list.id)) throw new Error("Duplicate Cuelist identity");
		ids.add(list.id);
		if (list.pool_number != null) {
			claim(list.pool_number, list);
			canonical.set(list.id, list.pool_number);
			const aliases = new Set<number>();
			for (const alias of list.legacy_pool_aliases ?? []) {
				if (alias === list.pool_number || aliases.has(alias))
					throw new Error("Cuelist aliases must be distinct");
				aliases.add(alias);
				claim(alias, list);
			}
		} else {
			if (list.legacy_pool_aliases?.length)
				throw new Error("Cuelist aliases require a canonical number");

		}
	}
	for (const list of [...lists].sort((a, b) => a.id.localeCompare(b.id))) {
		if (canonical.has(list.id)) continue;
		let number = 1;
		while (number <= 1000 && owners.has(number)) number++;
		claim(number, list);
		canonical.set(list.id, number);
	}
	return [...owners]
		.sort(([a], [b]) => a - b)
		.map(([number, cueList]) => ({
			number,
			canonicalNumber: canonical.get(cueList.id)!,
			legacyAlias: canonical.get(cueList.id) !== number,
			cueList,
			assignment: playbacks.find(
				(playback) =>
					playback.target.type === "cue_list" &&
					playback.target.cue_list_id === cueList.id,
			),
		}));
}
