import {
	type DirectValueChoice,
	discreteProgrammerTarget,
	formatDiscreteValues,
	formatNormalizedRange,
	formatNormalizedValue,
	normalizedProgrammerTarget,
} from "./model";
import type { ParameterProjection } from "./useParameterProjection";

function fixtureEntry(
	projection: ParameterProjection,
	fixtureId: string,
	attribute: string,
) {
	return projection.programmerValues.find(
		(candidate) =>
			candidate.fixtureId === fixtureId && candidate.attribute === attribute,
	);
}

function groupEntry(projection: ParameterProjection, attribute: string) {
	return projection.groupProgrammerValues.find(
		(candidate) => candidate.attribute === attribute,
	);
}

export function normalizedParameterTarget(
	projection: ParameterProjection,
	attribute: string,
) {
	if (projection.selectedGroupId)
		return normalizedProgrammerTarget(groupEntry(projection, attribute)?.value);
	for (const fixtureId of projection.selectedFixtureIds) {
		const target = normalizedProgrammerTarget(
			fixtureEntry(projection, fixtureId, attribute)?.value,
		);
		if (target != null) return target;
	}
}

export function discreteParameterTarget(
	projection: ParameterProjection,
	attribute: string,
) {
	if (projection.selectedGroupId)
		return discreteProgrammerTarget(groupEntry(projection, attribute)?.value);
	for (const fixtureId of projection.selectedFixtureIds) {
		const target = discreteProgrammerTarget(
			fixtureEntry(projection, fixtureId, attribute)?.value,
		);
		if (target != null) return target;
	}
}

export function normalizedParameterDisplay(
	projection: ParameterProjection,
	attribute: string,
) {
	if (projection.selectedGroupId) {
		const target = normalizedParameterTarget(projection, attribute);
		return target == null ? undefined : formatNormalizedValue(target);
	}
	return formatNormalizedRange(
		projection.selectedFixtureIds.flatMap((fixtureId) => {
			const target = normalizedProgrammerTarget(
				fixtureEntry(projection, fixtureId, attribute)?.value,
			);
			const value =
				target ?? projection.normalizedByFixture.get(fixtureId)?.get(attribute);
			return value == null ? [] : [value];
		}),
	);
}

export function discreteParameterDisplay(
	projection: ParameterProjection,
	attribute: string,
) {
	if (projection.selectedGroupId)
		return discreteParameterTarget(projection, attribute);
	return formatDiscreteValues(
		projection.selectedFixtureIds.flatMap((fixtureId) => {
			const target = discreteProgrammerTarget(
				fixtureEntry(projection, fixtureId, attribute)?.value,
			);
			const value =
				target ?? projection.discreteByFixture.get(fixtureId)?.get(attribute);
			return value == null ? [] : [value];
		}),
	);
}

export function directParameterChoiceActive(
	projection: ParameterProjection,
	choice: DirectValueChoice,
) {
	return choice.assignments.some((assignment) =>
		projection.programmerValues.some(
			(entry) =>
				entry.fixtureId === assignment.fixtureId &&
				entry.attribute === assignment.attribute &&
				discreteProgrammerTarget(entry.value) === choice.semanticId,
		),
	);
}

export function hasParameterValue(
	projection: ParameterProjection,
	attribute: string,
) {
	return projection.selectedGroupId
		? projection.groupProgrammerValues.some(
				(entry) => entry.attribute === attribute,
			)
		: projection.programmerValues.some(
				(entry) => entry.attribute === attribute,
			);
}
