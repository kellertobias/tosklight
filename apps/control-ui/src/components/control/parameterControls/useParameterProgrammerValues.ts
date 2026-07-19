import { useCallback, useMemo } from "react";
import type { AttributeValue } from "../../../api/types/playback";
import type {
	ProgrammerFixtureValue,
	ProgrammerGroupValue,
} from "../../../features/programmerValues/contracts";
import { useProgrammerValuesSelector } from "../../../features/programmerValues/ProgrammerValuesView";
import type { ProgrammerValuesState } from "../../../features/programmerValues/store";

export interface ParameterProgrammerValuesView {
	ready: boolean;
	fixtureValues: readonly ProgrammerFixtureValue[];
	groupValues: readonly ProgrammerGroupValue[];
}

const EMPTY_VALUES = Object.freeze([]) as readonly never[];
const PENDING_VIEW: ParameterProgrammerValuesView = Object.freeze({
	ready: false,
	fixtureValues: EMPTY_VALUES,
	groupValues: EMPTY_VALUES,
});

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

function selectParameterValues(
	state: ProgrammerValuesState,
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

function equalParameterValues(
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
	left: ProgrammerFixtureValue,
	right: ProgrammerFixtureValue,
) {
	return (
		left.fixtureId === right.fixtureId &&
		left.attribute === right.attribute &&
		equalProgrammerValue(left, right)
	);
}

function equalGroupValue(
	left: ProgrammerGroupValue,
	right: ProgrammerGroupValue,
) {
	return (
		left.groupId === right.groupId &&
		left.attribute === right.attribute &&
		equalProgrammerValue(left, right)
	);
}

function equalProgrammerValue(
	left: ProgrammerFixtureValue | ProgrammerGroupValue,
	right: ProgrammerFixtureValue | ProgrammerGroupValue,
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
