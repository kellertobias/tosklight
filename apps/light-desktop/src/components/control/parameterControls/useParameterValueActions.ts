import { useMemo } from "react";
import { useProgrammerPreloadValuesActions } from "../../../features/programmerPreloadValues/ProgrammerPreloadValuesView";
import { LatestProgrammerValuesWriteQueue } from "../../../features/programmerValues/LatestProgrammerValuesWriteQueue";
import { useProgrammerValuesActions } from "../../../features/programmerValues/ProgrammerValuesView";
import { useStrictModeSafeStop } from "../../../features/shared/useStrictModeSafeStop";
import {
	type ParameterValuesMutationPort,
	parameterMutationKey,
	releaseParameterMutations,
	setParameterMutations,
	setParameterRangeMutations,
	submitParameterAbsoluteIntent,
	submitParameterStep,
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
	const applyParameter = (
		attribute: string,
		level: number,
		undoGroup?: string | null,
	) => {
		if (
			projection.programmerValuesRoute === "normal" &&
			!projection.selectedGroupId &&
			actions?.applyIntent
		)
			return queue.submitLatest(
				`intent:${projection.selectedFixtureIds.join(",")}:${attribute}`,
				String(level),
				() =>
					submitParameterAbsoluteIntent(
						canWriteValues ? actions : null,
						projection,
						attribute,
						{ kind: "normalized", value: level },
						() => crypto.randomUUID(),
						undoGroup,
					) ?? Promise.resolve(null),
			);
		const mutations = setParameterMutations(
			projection,
			attribute,
			{ kind: "normalized", value: level },
			projection.programmerValuesRoute === "normal"
				? { fade: false, fadeMillis: null, delayMillis: null }
				: undefined,
		);
		return queue.submitLatest(
			parameterMutationKey(mutations),
			JSON.stringify(mutations),
			() => submit(mutations),
		);
	};
	const stepParameter = (
		attribute: string,
		delta: number,
		undoGroup?: string | null,
	) =>
		queue.submitBarrier(() =>
			submitParameterStep(
				canWriteValues ? actions : null,
				projection,
				attribute,
				delta,
				() => crypto.randomUUID(),
				undoGroup,
			),
		);
	return {
		canWriteValues,
		relativeSteps: Boolean(
			projection.programmerValuesRoute === "normal" && actions?.applyIntent,
		),
		applyParameter,
		stepParameter,
		applyParameterRange: (attribute: string, percentages: number[]) =>
			queue.submitBarrier(
				() =>
					(projection.programmerValuesRoute === "normal" &&
						submitParameterAbsoluteIntent(
							canWriteValues ? actions : null,
							projection,
							attribute,
							{
								kind: "spread",
								value: percentages.map(
									(value) => Math.max(0, Math.min(100, value)) / 100,
								),
							},
						)) ||
					submit(setParameterRangeMutations(projection, attribute, percentages)),
			),
		releaseParameter: (attribute: string) =>
			queue.submitBarrier(() =>
				submit(releaseParameterMutations(projection, attribute)),
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
