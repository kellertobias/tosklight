import type {
	MacroExecution,
	RunningTimecodeDefinition,
	TimecodeRuntime,
} from "../../api/runtimeModels";
import type { VersionedObject } from "../../api/types";
import type { RunningDynamicController } from "../../components/modals/systemControls/runningDynamicsAuthority";
import type { RunningCueListSource } from "../../components/modals/systemControls/runningPlaybackAuthority";

export const RUNNING_KINDS = [
	"cue_list",
	"dynamic",
	"timecode",
	"macro",
] as const;

export type RunningKind = (typeof RUNNING_KINDS)[number];
export type RunningFilter = "all" | RunningKind;

interface RunningRowBase {
	key: string;
	kind: RunningKind;
	number: number | null;
	name: string;
	status: string;
	cueNumber: number | null;
	off(): Promise<unknown>;
}

export interface RunningCueListRow extends RunningRowBase {
	kind: "cue_list";
	source: RunningCueListSource;
}

export interface RunningDynamicRow extends RunningRowBase {
	kind: "dynamic";
	controller: RunningDynamicController;
}

export interface RunningTimecodeRow extends RunningRowBase {
	kind: "timecode";
	snapshot: TimecodeRuntime;
}

export interface RunningMacroRow extends RunningRowBase {
	kind: "macro";
	execution: MacroExecution;
}

export type RunningRow =
	| RunningCueListRow
	| RunningDynamicRow
	| RunningTimecodeRow
	| RunningMacroRow;

export interface RunningModelInput {
	playbacks: readonly RunningCueListSource[];
	dynamics: readonly RunningDynamicController[];
	timecodes: readonly TimecodeRuntime[];
	timecodeDefinitions?: readonly VersionedObject<RunningTimecodeDefinition>[];
	macros: readonly MacroExecution[];
	releasePlayback(source: RunningCueListSource): Promise<unknown>;
	turnOffDynamic(controller: RunningDynamicController): Promise<unknown>;
	stopTimecode(timecodeId: string): Promise<unknown>;
	cancelMacro(executionId: string): Promise<unknown>;
}

/**
 * Projects runtime authorities into one row per independently stoppable object.
 * The callback captured by each row is the single authoritative action for its identity.
 */
export function buildRunningRows(input: RunningModelInput): RunningRow[] {
	const rows: RunningRow[] = [];
	const cueListSources = new Map<string, RunningCueListSource>();
	for (const source of input.playbacks) {
		const existing = cueListSources.get(source.cueListId);
		if (!existing || source.identity.kind === "cue_list")
			cueListSources.set(source.cueListId, source);
	}
	for (const source of cueListSources.values()) {
		const cueNumber =
			source.cue?.number ??
			source.runtime.current?.number ??
			source.runtime.cue_index + 1;
		rows.push({
			key: `cue-list:${source.cueListId}`,
			kind: "cue_list",
			// Pool identity and runtime-control identity are deliberately separate. A
			// direct Cuelist release stops the shared logical runtime regardless of the
			// software, hardware, OSC, or Playback surface that started it.
			number: source.cueListNumber ?? source.playbackNumber,
			name: source.cueList?.name ?? source.label,
			status: source.runtime.paused ? "Paused" : "Running",
			cueNumber,
			source,
			off: () => input.releasePlayback(source),
		});
	}

	const seenDynamicControllers = new Set<string>();
	for (const controller of input.dynamics) {
		if (
			isContainedDynamicSource(controller.source) ||
			seenDynamicControllers.has(controller.controllerId)
		)
			continue;
		seenDynamicControllers.add(controller.controllerId);
		rows.push({
			key: `dynamic:${controller.controllerId}`,
			kind: "dynamic",
			number: controller.poolNumber,
			name: controller.name,
			status:
				controller.paused || controller.instancePaused ? "Paused" : "Running",
			cueNumber: null,
			controller,
			off: () => input.turnOffDynamic(controller),
		});
	}

	const timecodeDefinitions = new Map(
		(input.timecodeDefinitions ?? []).map((object) => [
			object.body.id,
			object.body,
		]),
	);
	const seenTimecodes = new Set<string>();
	for (const snapshot of input.timecodes) {
		if (snapshot.state === "stopped" || seenTimecodes.has(snapshot.timecode_id))
			continue;
		seenTimecodes.add(snapshot.timecode_id);
		const definition = timecodeDefinitions.get(snapshot.timecode_id);
		rows.push({
			key: `timecode:${snapshot.timecode_id}`,
			kind: "timecode",
			number: definition?.number ?? null,
			name: definition?.name ?? `Timecode ${snapshot.timecode_id.slice(0, 8)}`,
			status: snapshot.state === "paused" ? "Paused" : "Running",
			cueNumber: null,
			snapshot,
			off: () => input.stopTimecode(snapshot.timecode_id),
		});
	}

	const seenMacroExecutions = new Set<string>();
	for (const execution of input.macros) {
		if (
			!isActiveMacro(execution) ||
			seenMacroExecutions.has(execution.execution_id)
		)
			continue;
		seenMacroExecutions.add(execution.execution_id);
		rows.push({
			key: `macro:${execution.execution_id}`,
			kind: "macro",
			number: execution.macro_number,
			name: execution.macro_name,
			status: macroStatus(execution.state),
			cueNumber: null,
			execution,
			off: () => input.cancelMacro(execution.execution_id),
		});
	}

	return rows;
}

export function filterRunningRows(
	rows: readonly RunningRow[],
	filter: RunningFilter,
): RunningRow[] {
	return filter === "all"
		? [...rows]
		: rows.filter((row) => row.kind === filter);
}

export function runningKindLabel(kind: RunningKind): string {
	switch (kind) {
		case "cue_list":
			return "Cuelists";
		case "dynamic":
			return "Dynamics";
		case "timecode":
			return "Timecodes";
		case "macro":
			return "Macros";
	}
}

function isContainedDynamicSource(source: string): boolean {
	return source === "Cue" || source.startsWith("Playback ");
}

function isActiveMacro(execution: MacroExecution): boolean {
	return ["queued", "validating", "running"].includes(execution.state);
}

function macroStatus(state: MacroExecution["state"]): string {
	switch (state) {
		case "queued":
			return "Queued";
		case "validating":
			return "Validating";
		case "running":
			return "Running";
		default:
			return state;
	}
}
