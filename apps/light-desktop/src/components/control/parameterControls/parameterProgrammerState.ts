import {
	discreteProgrammerTarget,
	formatDiscreteValues,
	formatNormalizedRange,
	formatNormalizedValue,
	normalizedProgrammerTarget,
} from "./model";
import { formatPositionMovement } from "./positionMovement";
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
	const format = (value: string) =>
		attribute === "position.movement"
			? formatPositionMovement(value, projection.movementRepresentation)
			: value;
	if (projection.selectedGroupId) {
		const target = normalizedParameterTarget(projection, attribute);
		return target == null ? undefined : format(formatNormalizedValue(target));
	}
	const range = formatNormalizedRange(
		projection.selectedFixtureIds.flatMap((fixtureId) => {
			const target = normalizedProgrammerTarget(
				fixtureEntry(projection, fixtureId, attribute)?.value,
			);
			const value =
				target ?? projection.normalizedByFixture.get(fixtureId)?.get(attribute);
			return value == null ? [] : [value];
		}),
	);
	return range == null ? undefined : format(range);
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
