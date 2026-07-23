import type { AttributeValue } from "../../../api/types";
import type {
	BatchProgrammerValuesInput,
	ProgrammerValuesMutation,
	ProgrammerValueTiming,
} from "../../../features/programmerValues/contracts";
import type { DirectValueChoice } from "./model";
import type { ParameterProjection } from "./useParameterProjection";

export interface ParameterValuesMutationPort {
	batch(input: BatchProgrammerValuesInput): Promise<unknown>;
}

export function parameterValueTiming(
	programmerFadeMillis: number | undefined,
): ProgrammerValueTiming {
	return {
		fade: true,
		fadeMillis: programmerFadeMillis ?? 3_000,
		delayMillis: null,
	};
}

export function setParameterMutations(
	projection: ParameterProjection,
	attribute: string,
	value: AttributeValue,
) {
	const timing = parameterValueTiming(
		projection.programmerFadeMillis,
	);
	if (projection.selectedGroupId)
		return [
			{
				action: "set_group",
				groupId: projection.selectedGroupId,
				attribute,
				value,
				timing,
			},
		] satisfies ProgrammerValuesMutation[];
	return projection.selectedFixtureIds.map(
		(fixtureId): ProgrammerValuesMutation => ({
			action: "set_fixture",
			fixtureId,
			attribute,
			value,
			timing,
		}),
	);
}

export function setParameterRangeMutations(
	projection: ParameterProjection,
	attribute: string,
	percentages: readonly number[],
) {
	const points = percentages.map(normalizePercentage);
	if (projection.selectedGroupId)
		return setParameterMutations(projection, attribute, {
			kind: "spread",
			value: points,
		});
	if (projection.selectedFixtureIds.length === 0) return [];
	// The server resolves the per-fixture interpolation across the ordered selection.
	return [
		{
			action: "set_selection",
			fixtureIds: projection.selectedFixtureIds,
			attribute,
			value: { kind: "spread", value: points },
			timing: parameterValueTiming(projection.programmerFadeMillis),
		},
	] satisfies ProgrammerValuesMutation[];
}

export function releaseParameterMutations(
	projection: ParameterProjection,
	attribute: string,
) {
	if (projection.selectedGroupId)
		return projection.groupProgrammerValues.some(
			(entry) => entry.attribute === attribute,
		)
			? ([
					{
						action: "release_group",
						groupId: projection.selectedGroupId,
						attribute,
					},
				] satisfies ProgrammerValuesMutation[])
			: [];
	const valuedFixtures = new Set(
		projection.programmerValues
			.filter((entry) => entry.attribute === attribute)
			.map((entry) => entry.fixtureId),
	);
	return projection.selectedFixtureIds.flatMap((fixtureId) =>
		valuedFixtures.has(fixtureId)
			? ([
					{
						action: "release_fixture",
						fixtureId,
						attribute,
					},
				] satisfies ProgrammerValuesMutation[])
			: [],
	);
}

export function directValueMutations(
	projection: ParameterProjection,
	choice: DirectValueChoice,
) {
	const timing = parameterValueTiming(
		projection.programmerFadeMillis,
	);
	return choice.assignments.map(
		(assignment): ProgrammerValuesMutation => ({
			action: "set_fixture",
			fixtureId: assignment.fixtureId,
			attribute: assignment.attribute,
			value: { kind: "discrete", value: choice.semanticId },
			timing,
		}),
	);
}

export function submitParameterMutations(
	actions: ParameterValuesMutationPort | null,
	mutations: readonly ProgrammerValuesMutation[],
	requestId: () => string = () => crypto.randomUUID(),
) {
	if (!actions || mutations.length === 0) return Promise.resolve(null);
	return actions.batch({ requestId: requestId(), mutations });
}

export function parameterMutationKey(
	mutations: readonly ProgrammerValuesMutation[],
) {
	return mutations
		.map((mutation) => {
			if (mutation.action === "set_group")
				return `group:${mutation.groupId}:${mutation.attribute}`;
			if (mutation.action === "set_fixture")
				return `fixture:${mutation.fixtureId}:${mutation.attribute}`;
			if (mutation.action === "set_selection")
				return `selection:${mutation.fixtureIds.join(",")}:${mutation.attribute}`;
			return mutation.action;
		})
		.join("\u0000");
}

function normalizePercentage(value: number) {
	return Math.max(0, Math.min(100, value)) / 100;
}
