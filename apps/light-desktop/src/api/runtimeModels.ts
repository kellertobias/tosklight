import type {
	EventPayload,
	MacroExecutionSnapshot as WireMacroExecutionSnapshot,
	TimecodeTransportSnapshot as WireTimecodeTransportSnapshot,
} from "./generated/light-wire";

export type MacroExecutionState =
	| "queued"
	| "validating"
	| "running"
	| "succeeded"
	| "failed"
	| "cancelled";

export interface MacroExecution {
	execution_id: string;
	macro_id: string;
	macro_number: number;
	macro_name: string;
	source_revision: number;
	desk_id: string;
	user_id: string;
	session_id: string;
	state: MacroExecutionState;
	line?: number | null;
	command?: string | null;
	message?: string | null;
	trigger: WireMacroExecutionSnapshot["trigger"];
	started_at: string;
	finished_at?: string | null;
}

export interface MacroRuntime {
	desk_id: string;
	active: MacroExecution[];
	recent: MacroExecution[];
}

export interface TimecodeRuntime {
	timecode_id: string;
	revision: number;
	state: "stopped" | "playing" | "paused";
	frame: number;
	duration_frame: number;
	audio_linked: boolean;
	cue_list_clips?: WireTimecodeTransportSnapshot["cue_list_clips"];
}

export interface RunningTimecodeDefinition {
	id: string;
	number: number;
	name: string;
}

export type SupplementalRuntimeEvent =
	| { type: "macro_execution_changed"; execution: MacroExecution }
	| { type: "timecode_runtime_changed"; snapshot: TimecodeRuntime };

export interface SupplementalEventSource {
	onEvent(listener: (event: SupplementalRuntimeEvent) => void): () => unknown;
}

export function macroExecutionFromWire(
	value: WireMacroExecutionSnapshot,
): MacroExecution {
	return { ...value };
}

export function timecodeRuntimeFromWire(
	value: WireTimecodeTransportSnapshot,
): TimecodeRuntime {
	return { ...value };
}

export function supplementalEventSource(source: {
	onEvent(listener: (event: EventPayload) => void): () => unknown;
}): SupplementalEventSource {
	return {
		onEvent: (listener) =>
			source.onEvent((event) => {
				if (event.type === "macro_execution_changed") {
					listener({
						type: event.type,
						execution: macroExecutionFromWire(event.execution),
					});
				} else if (event.type === "timecode_runtime_changed") {
					listener({
						type: event.type,
						snapshot: timecodeRuntimeFromWire(event.snapshot),
					});
				}
			}),
	};
}
