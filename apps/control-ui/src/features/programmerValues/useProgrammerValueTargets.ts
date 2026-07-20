import { useProgrammerCaptureModeSelector } from "../programmerCaptureMode/ProgrammerCaptureModeView";
import { useProgrammerPreloadValuesSelector } from "../programmerPreloadValues/ProgrammerPreloadValuesView";
import type { ProgrammerPreloadValuesState } from "../programmerPreloadValues/store";
import { useProgrammerValuesSelector } from "./ProgrammerValuesView";
import type { ProgrammerValuesState } from "./store";
import { selectValuesAuthority } from "./useProgrammerValuesActivity";

export interface ProgrammerValueTargets {
	fixtureIds: readonly string[];
	groupIds: readonly string[];
}

/** Exact active value addresses without subscribing to value content or timing changes. */
export function useProgrammerValueTargets(enabled = true) {
	const selectedAuthority = useProgrammerCaptureModeSelector(
		selectValuesAuthority,
		Object.is,
		enabled,
	);
	const authority = selectedAuthority ?? "loading";
	const normalTargets = useProgrammerValuesSelector(
		selectNormalTargets,
		sameTargets,
		enabled && authority === "normal",
	);
	const pendingTargets = useProgrammerPreloadValuesSelector(
		selectPendingTargets,
		sameTargets,
		enabled && authority === "preload",
	);
	if (authority === "normal") return normalTargets;
	if (authority === "preload") return pendingTargets;
	return null;
}

function selectNormalTargets(state: ProgrammerValuesState) {
	return readyTargets(state);
}

function selectPendingTargets(state: ProgrammerPreloadValuesState) {
	return readyTargets(state);
}

function readyTargets(state: {
	status: string;
	repairRequired: boolean;
	projection: {
		fixtureValues: readonly { fixtureId: string }[];
		groupValues: readonly { groupId: string }[];
	} | null;
}): ProgrammerValueTargets | null {
	if (state.status !== "ready" || state.repairRequired || !state.projection)
		return null;
	return {
		fixtureIds: uniqueSorted(
			state.projection.fixtureValues.map((value) => value.fixtureId),
		),
		groupIds: uniqueSorted(
			state.projection.groupValues.map((value) => value.groupId),
		),
	};
}

function uniqueSorted(values: readonly string[]) {
	return [...new Set(values)].sort();
}

function sameTargets(
	left: ProgrammerValueTargets | null,
	right: ProgrammerValueTargets | null,
) {
	if (left === null || right === null) return left === right;
	return (
		sameIds(left.fixtureIds, right.fixtureIds) &&
		sameIds(left.groupIds, right.groupIds)
	);
}

function sameIds(left: readonly string[], right: readonly string[]) {
	return (
		left.length === right.length &&
		left.every((value, index) => value === right[index])
	);
}
