import { useCallback, useMemo } from "react";
import { useProgrammerPreloadValuesSelector } from "../../../features/programmerPreloadValues/ProgrammerPreloadValuesView";
import type { ProgrammerPreloadValuesState } from "../../../features/programmerPreloadValues/store";
import {
	equalParameterValues,
	type ParameterProgrammerValuesView,
	selectParameterValues,
} from "./parameterValuesView";

export function useParameterPreloadValues(
	selectedFixtureIds: readonly string[],
	selectedGroupId: string | null,
	enabled: boolean,
): ParameterProgrammerValuesView | null {
	const fixtureKey = [...new Set(selectedFixtureIds)].sort().join("\u0000");
	const fixtureIds = useMemo(
		() => new Set(selectedFixtureIds),
		// The canonical key owns set equality across render-created arrays.
		// eslint-disable-next-line react-hooks/exhaustive-deps
		[fixtureKey],
	);
	const selector = useCallback(
		(state: ProgrammerPreloadValuesState) =>
			selectParameterValues(state, fixtureIds, selectedGroupId),
		[fixtureIds, selectedGroupId],
	);
	return useProgrammerPreloadValuesSelector(
		selector,
		equalParameterValues,
		enabled,
	);
}
