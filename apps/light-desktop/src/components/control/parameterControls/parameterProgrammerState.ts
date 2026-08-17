import {
	discreteProgrammerTarget,
	formatDiscreteValues,
	formatNormalizedRange,
	formatNormalizedValue,
	normalizedProgrammerTarget,
} from "./model";
import { formatPositionAxis, formatPositionMovement } from "./positionMovement";
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

export function parameterSemanticDisplay(
	projection: ParameterProjection,
	attribute: string,
) {
	const values = projection.dynamicProgrammerValues.filter(
		(candidate) => candidate.attribute === attribute,
	);
	if (values.some((candidate) => candidate.value.type === "release"))
		return "Release";
	if (
		values.some(
			(candidate) =>
				candidate.value.type === "fix_at" || candidate.value.type === "static",
		)
	)
		return "FixAT";
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
	const mediaAddressLabel =
		attribute === "media.file" || attribute === "media.mask.file"
			? "File"
			: attribute === "media.folder" || attribute === "media.mask.folder"
				? "Folder"
				: null;
	const formatMediaAddresses = (values: number[]) => {
		if (!mediaAddressLabel || !values.length) return undefined;
		const addresses = values.map((value) =>
			Math.max(0, Math.min(255, Math.round(value * 255))),
		);
		const minimum = Math.min(...addresses);
		const maximum = Math.max(...addresses);
		if (minimum === maximum) {
			if (mediaAddressLabel === "File" && minimum === 0) return "No file";
			return `${mediaAddressLabel} ${minimum}`;
		}
		return `${mediaAddressLabel} ${minimum}…${maximum}`;
	};
	const format = (value: string) => {
		if (attribute === "position.movement")
			return formatPositionMovement(value, projection.movementRepresentation);
		if (attribute === "pan")
			return formatPositionAxis(value, projection.panRepresentation);
		if (attribute === "tilt")
			return formatPositionAxis(value, projection.tiltRepresentation);
		return value;
	};
	if (projection.selectedGroupId) {
		const target = normalizedParameterTarget(projection, attribute);
		return target == null
			? undefined
			: (formatMediaAddresses([target]) ??
					format(formatNormalizedValue(target)));
	}
	const values = projection.selectedFixtureIds.flatMap((fixtureId) => {
		const target = normalizedProgrammerTarget(
			fixtureEntry(projection, fixtureId, attribute)?.value,
		);
		const value =
			target ?? projection.normalizedByFixture.get(fixtureId)?.get(attribute);
		return value == null ? [] : [value];
	});
	const range = formatMediaAddresses(values) ?? formatNormalizedRange(values);
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
