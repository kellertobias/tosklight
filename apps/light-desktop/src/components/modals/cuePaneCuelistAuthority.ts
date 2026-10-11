import { useMemo } from "react";
import { useCueLists } from "../../features/showObjects/ShowObjectsState";
import { useShowObjectView } from "../../features/showObjects/ShowObjectsView";
export interface CuePaneCuelistPlayback { number: number; name: string }
/** Cues pick their own Cuelist Pool identity, independently of Playback assignments. */
export function useCuePaneCuelistPlaybacks(enabled: boolean): readonly CuePaneCuelistPlayback[] {
	useShowObjectView("cue_list", enabled);
	const lists = useCueLists(enabled);
	return useMemo(() => enabled ? lists.flatMap(({body}) => body.pool_number == null ? [] : [{number: body.pool_number, name: body.name}]).sort((left, right) => left.number - right.number) : [], [enabled, lists]);
}
