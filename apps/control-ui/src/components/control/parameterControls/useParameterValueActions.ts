import { useMemo } from "react";
import { useProgrammerPreloadValuesActions } from "../../../features/programmerPreloadValues/ProgrammerPreloadValuesView";
import { LatestProgrammerValuesWriteQueue } from "../../../features/programmerValues/LatestProgrammerValuesWriteQueue";
import { useProgrammerValuesActions } from "../../../features/programmerValues/ProgrammerValuesView";
import { useStrictModeSafeStop } from "../../../features/shared/useStrictModeSafeStop";
import type { DirectValueChoice } from "./model";
import {
	directValueMutations,
	type ParameterValuesMutationPort,
	parameterMutationKey,
	releaseParameterMutations,
	setParameterMutations,
	setParameterRangeMutations,
	submitParameterMutations,
} from "./parameterValueMutations";
import type { ParameterProjection } from "./useParameterProjection";

export function useParameterValueActions(projection: ParameterProjection) {
	const normalActions = useProgrammerValuesActions();
	const preloadActions = useProgrammerPreloadValuesActions();
	const actions = selectActions(
		projection.programmerValuesRoute,
		normalActions,
		preloadActions,
	);
	const queue = useMemo(
		() => new LatestProgrammerValuesWriteQueue(),
		[actions],
	);
	useStrictModeSafeStop(queue);
	const canWriteValues = projection.programmerValuesReady && actions !== null;
	const submit = (mutations: ReturnType<typeof setParameterMutations>) =>
		submitParameterMutations(canWriteValues ? actions : null, mutations);
	const applyParameter = (attribute: string, level: number) => {
		const mutations = setParameterMutations(projection, attribute, {
			kind: "normalized",
			value: level,
		});
		return queue.submitLatest(
			parameterMutationKey(mutations),
			JSON.stringify(mutations),
			() => submit(mutations),
		);
	};
	return {
		canWriteValues,
		applyParameter,
		applyParameterRange: (attribute: string, percentages: number[]) =>
			queue.submitBarrier(() =>
				submit(setParameterRangeMutations(projection, attribute, percentages)),
			),
		releaseParameter: (attribute: string) =>
			queue.submitBarrier(() =>
				submit(releaseParameterMutations(projection, attribute)),
			),
		applyDirectValue: (choice: DirectValueChoice) =>
			queue.submitBarrier(() =>
				submit(directValueMutations(projection, choice)),
			),
	};
}

function selectActions(
	route: ParameterProjection["programmerValuesRoute"],
	normal: ParameterValuesMutationPort | null,
	preload: ParameterValuesMutationPort | null,
) {
	if (route === "normal") return normal;
	if (route === "preload") return preload;
	return null;
}
