import type { AttributeValue } from "../../../api/types/playback";

interface ParameterValueTiming {
	fade: boolean;
	fadeMillis: number | null;
	delayMillis: number | null;
	programmerOrder: number;
}

export interface ParameterFixtureValue extends ParameterValueTiming {
	fixtureId: string;
	attribute: string;
	value: AttributeValue;
}

export interface ParameterGroupValue extends ParameterValueTiming {
	groupId: string;
	attribute: string;
	value: AttributeValue;
}

export interface ParameterProgrammerValuesView {
	ready: boolean;
	fixtureValues: readonly ParameterFixtureValue[];
	groupValues: readonly ParameterGroupValue[];
}

interface ParameterValuesState {
	status: string;
	projection: {
		fixtureValues: readonly ParameterFixtureValue[];
		groupValues: readonly ParameterGroupValue[];
	} | null;
}

const EMPTY_VALUES = Object.freeze([]) as readonly never[];
const PENDING_VIEW: ParameterProgrammerValuesView = Object.freeze({
	ready: false,
	fixtureValues: EMPTY_VALUES,
	groupValues: EMPTY_VALUES,
});

export function selectParameterValues(
	state: ParameterValuesState,
	fixtureIds: ReadonlySet<string>,
	groupId: string | null,
): ParameterProgrammerValuesView {
	if (state.status !== "ready" || !state.projection) return PENDING_VIEW;
	return {
		ready: true,
		fixtureValues: state.projection.fixtureValues.filter((value) =>
			fixtureIds.has(value.fixtureId),
		),
		groupValues: groupId
			? state.projection.groupValues.filter(
					(value) => value.groupId === groupId,
				)
			: EMPTY_VALUES,
	};
}

export function equalParameterValues(
	left: ParameterProgrammerValuesView,
	right: ParameterProgrammerValuesView,
) {
	return (
		left.ready === right.ready &&
		equalValues(left.fixtureValues, right.fixtureValues, equalFixtureValue) &&
		equalValues(left.groupValues, right.groupValues, equalGroupValue)
	);
}

function equalValues<T>(
	left: readonly T[],
	right: readonly T[],
	equal: (left: T, right: T) => boolean,
) {
	return (
		left.length === right.length &&
		left.every((value, index) => equal(value, right[index]))
	);
}

function equalFixtureValue(
	left: ParameterFixtureValue,
	right: ParameterFixtureValue,
) {
	return (
		left.fixtureId === right.fixtureId &&
		left.attribute === right.attribute &&
		equalProgrammerValue(left, right)
	);
}

function equalGroupValue(
	left: ParameterGroupValue,
	right: ParameterGroupValue,
) {
	return (
		left.groupId === right.groupId &&
		left.attribute === right.attribute &&
		equalProgrammerValue(left, right)
	);
}

function equalProgrammerValue(
	left: ParameterFixtureValue | ParameterGroupValue,
	right: ParameterFixtureValue | ParameterGroupValue,
) {
	return (
		left.programmerOrder === right.programmerOrder &&
		left.fade === right.fade &&
		left.fadeMillis === right.fadeMillis &&
		left.delayMillis === right.delayMillis &&
		equalAttributeValue(left.value, right.value)
	);
}

function equalAttributeValue(left: AttributeValue, right: AttributeValue) {
	if (left.kind !== right.kind) return false;
	switch (left.kind) {
		case "spread":
			return (
				right.kind === "spread" &&
				equalValues(left.value, right.value, Object.is)
			);
		case "color_xyz":
			return (
				right.kind === "color_xyz" &&
				left.value.x === right.value.x &&
				left.value.y === right.value.y &&
				left.value.z === right.value.z
			);
		default:
			return right.kind === left.kind && left.value === right.value;
	}
}
