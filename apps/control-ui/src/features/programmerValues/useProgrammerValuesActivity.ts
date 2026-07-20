import { capturesProgrammerWrites } from "../programmerCaptureMode/contracts";
import { useProgrammerCaptureModeSelector } from "../programmerCaptureMode/ProgrammerCaptureModeView";
import type { ProgrammerCaptureModeState } from "../programmerCaptureMode/store";
import { useProgrammerPreloadValuesSelector } from "../programmerPreloadValues/ProgrammerPreloadValuesView";
import type { ProgrammerPreloadValuesState } from "../programmerPreloadValues/store";
import { useProgrammerValuesSelector } from "./ProgrammerValuesView";
import type { ProgrammerValuesState } from "./store";

export type ProgrammerValuesAuthority = "loading" | "normal" | "preload";

export interface ProgrammerValuesActivity {
	authority: ProgrammerValuesAuthority;
	ready: boolean;
	valueCount: number;
	pendingValueCount: number;
}

/** Activates only the value authority selected by the exact-user capture projection. */
export function useProgrammerValuesActivity(
	enabled = true,
): ProgrammerValuesActivity {
	const selectedAuthority = useProgrammerCaptureModeSelector(
		selectValuesAuthority,
		Object.is,
		enabled,
	);
	const authority = selectedAuthority ?? "loading";
	const normalCount = useProgrammerValuesSelector(
		selectReadyNormalCount,
		Object.is,
		enabled && authority === "normal",
	);
	const pendingCount = useProgrammerPreloadValuesSelector(
		selectReadyPendingCount,
		Object.is,
		enabled && authority === "preload",
	);
	const activeCount = authority === "preload" ? pendingCount : normalCount;
	return {
		authority,
		ready: activeCount !== null,
		valueCount: activeCount ?? 0,
		pendingValueCount:
			authority === "preload" && pendingCount !== null ? pendingCount : 0,
	};
}

/** Exact current-user normal count, independent of active Preload capture. */
export function useNormalProgrammerValueCount(enabled = true) {
	return useProgrammerValuesSelector(
		selectReadyNormalCount,
		Object.is,
		enabled,
	);
}

export function selectValuesAuthority(
	state: ProgrammerCaptureModeState,
): ProgrammerValuesAuthority {
	if (
		state.status !== "ready" ||
		state.repairRequired ||
		state.projection === null
	)
		return "loading";
	return capturesProgrammerWrites(state.projection) ? "preload" : "normal";
}

function selectReadyNormalCount(state: ProgrammerValuesState) {
	return readyValueCount(state);
}

function selectReadyPendingCount(state: ProgrammerPreloadValuesState) {
	return readyValueCount(state);
}

function readyValueCount(state: {
	status: string;
	repairRequired: boolean;
	projection: {
		fixtureValues: readonly unknown[];
		groupValues: readonly unknown[];
	} | null;
}) {
	if (state.status !== "ready" || state.repairRequired || !state.projection)
		return null;
	return (
		state.projection.fixtureValues.length + state.projection.groupValues.length
	);
}
