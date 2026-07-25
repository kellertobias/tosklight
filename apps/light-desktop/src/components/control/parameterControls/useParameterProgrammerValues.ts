import { useCallback, useMemo } from "react";
import { useProgrammerValuesSelector } from "../../../features/programmerValues/ProgrammerValuesView";
import type { ProgrammerValuesState } from "../../../features/programmerValues/store";
import {
	equalParameterValues,
	type ParameterProgrammerValuesView,
	selectParameterValues,
} from "./parameterValuesView";

export type { ParameterProgrammerValuesView } from "./parameterValuesView";

export function useParameterProgrammerValues(
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
		(state: ProgrammerValuesState) =>
			selectParameterValues(state, fixtureIds, selectedGroupId),
		[fixtureIds, selectedGroupId],
	);
	return useProgrammerValuesSelector(selector, equalParameterValues, enabled);
}
