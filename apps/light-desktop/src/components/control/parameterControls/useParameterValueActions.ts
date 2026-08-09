import { useMemo } from "react";
import { useProgrammerPreloadValuesActions } from "../../../features/programmerPreloadValues/ProgrammerPreloadValuesView";
import { LatestProgrammerValuesWriteQueue } from "../../../features/programmerValues/LatestProgrammerValuesWriteQueue";
import { useProgrammerValuesActions } from "../../../features/programmerValues/ProgrammerValuesView";
import { useStrictModeSafeStop } from "../../../features/shared/useStrictModeSafeStop";
import {
	immediateParameterTiming,
	type ParameterValuesMutationPort,
	parameterMutationKey,
	parameterValueTiming,
	releaseParameterMutations,
	setParameterMutations,
	setParameterRangeMutations,
	submitParameterAbsoluteIntent,
	submitParameterMutations,
	submitParameterStep,
} from "./parameterValueMutations";
import type { ParameterProjection } from "./useParameterProjection";

type IndexedPresetTarget = {
	fixtureId: string;
	functionId?: string;
	profileRevision?: number;
};

function submitIndexedPreset(
	queue: LatestProgrammerValuesWriteQueue,
	actions: ParameterValuesMutationPort | null,
	canWriteValues: boolean,
	projection: ParameterProjection,
	attribute: string,
	semanticId: string,
	targets: ReadonlyArray<IndexedPresetTarget>,
) {
	return queue.submitBarrier(() => {
		const authoredTargets = targets.flatMap((target) =>
			target.functionId && target.profileRevision != null
				? [
						{
							fixtureId: target.fixtureId,
							functionId: target.functionId,
							expectedProfileRevision: target.profileRevision,
						},
					]
				: [],
		);
		if (
			actions?.applyIndexedPreset &&
			canWriteValues &&
			authoredTargets.length === targets.length &&
			authoredTargets.length
		)
			return actions.applyIndexedPreset({
				requestId: crypto.randomUUID(),
				expectedSelectionRevision: projection.selectionRevision,
				attribute,
				targets: authoredTargets,
			});
		return actions?.applyIntent && canWriteValues && targets.length
			? actions.applyIntent({
					requestId: crypto.randomUUID(),
					fixtureIds: targets.map((target) => target.fixtureId),
					attribute,
					operation: {
						type: "absolute_set",
						value: { kind: "discrete", value: semanticId },
					},
					timing:
						projection.programmerValuesRoute === "preload"
							? parameterValueTiming(projection.programmerFadeMillis)
							: immediateParameterTiming(),
				})
			: Promise.resolve(null);
	});
}

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
	const submit = (
		mutations: ReturnType<typeof setParameterMutations>,
		requestId?: string,
	) =>
		submitParameterMutations(
			canWriteValues ? actions : null,
			mutations,
			requestId ? () => requestId : undefined,
		);
	const applyParameter = (
		attribute: string,
		level: number,
		undoGroup?: string | null,
		requestId?: string,
	) => {
		if (actions?.applyIntent)
			return queue.submitLatest(
				`intent:${projection.selectedFixtureIds.join(",")}:${attribute}`,
				String(level),
				() =>
					submitParameterAbsoluteIntent(
						canWriteValues ? actions : null,
						projection,
						attribute,
						{ kind: "normalized", value: level },
						requestId ? () => requestId : undefined,
						undoGroup,
					) ?? Promise.resolve(null),
			);
		const mutations = setParameterMutations(
			projection,
			attribute,
			{ kind: "normalized", value: level },
			projection.programmerValuesRoute === "normal" &&
				!projection.directEntryUsesProgrammerFade
				? { fade: false, fadeMillis: null, delayMillis: null }
				: undefined,
		);
		return queue.submitLatest(
			parameterMutationKey(mutations),
			JSON.stringify(mutations),
			() => submit(mutations, requestId),
		);
	};
	const stepParameter = (
		attribute: string,
		delta: number,
		undoGroup?: string | null,
		requestId?: string,
	) =>
		queue.submitBarrier(() =>
			submitParameterStep(
				canWriteValues ? actions : null,
				projection,
				attribute,
				delta,
				requestId ? () => requestId : undefined,
				undoGroup,
			),
		);
	return {
		canWriteValues,
		relativeSteps: Boolean(actions?.applyIntent),
		applyParameter,
		applyIndexedPreset: (
			attribute: string,
			semanticId: string,
			targets: ReadonlyArray<IndexedPresetTarget>,
		) =>
			submitIndexedPreset(
				queue,
				actions,
				canWriteValues,
				projection,
				attribute,
				semanticId,
				targets,
			),
		stepParameter,
		applyParameterRange: (attribute: string, percentages: number[]) =>
			queue.submitBarrier(
				() =>
					(actions?.applyIntent &&
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
					submit(
						setParameterRangeMutations(projection, attribute, percentages),
					),
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
