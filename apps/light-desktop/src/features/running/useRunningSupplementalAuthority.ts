import { useCallback, useEffect, useRef, useState } from "react";
import type {
	MacroExecution,
	RunningTimecodeDefinition,
	TimecodeRuntime,
} from "../../api/runtimeModels";
import type { VersionedObject } from "../../api/types";
import type { RunningRuntimeActions } from "./RunningRuntimeActionsContext";

interface RunningSupplementalState {
	loading: boolean;
	error: string | null;
	macros: MacroExecution[];
	timecodes: TimecodeRuntime[];
	timecodeDefinitions: VersionedObject<RunningTimecodeDefinition>[];
}

const EMPTY_STATE: RunningSupplementalState = {
	loading: false,
	error: null,
	macros: [],
	timecodes: [],
	timecodeDefinitions: [],
};

/** Hydrates once, then follows ordered Macro and Timecode runtime projection events. */
export function useRunningSupplementalAuthority(
	enabled: boolean,
	showId: string | null,
	actions: RunningRuntimeActions | null,
) {
	const [state, setState] = useState<RunningSupplementalState>(EMPTY_STATE);
	const refreshRef = useRef<() => Promise<void>>(async () => undefined);

	useEffect(() => {
		if (!enabled || !showId || !actions) {
			refreshRef.current = async () => undefined;
			setState(EMPTY_STATE);
			return;
		}
		let mounted = true;
		const refresh = (): Promise<void> => {
			return (async () => {
				try {
					const [macroRuntime, timecodes, timecodeDefinitions] =
						await Promise.all([
							actions.macros.runtime(showId),
							actions.timecodes.runtime(showId),
							actions.showObjects.objects<RunningTimecodeDefinition>(
								showId,
								"timecode",
							),
						]);
					if (!mounted) return;
					setState((current) => ({
						loading: false,
						error: null,
						macros: mergeMacroExecutions(current.macros, macroRuntime.active),
						timecodes: mergeTimecodes(current.timecodes, timecodes),
						timecodeDefinitions,
					}));
				} catch (cause) {
					if (!mounted) return;
					setState((current) => ({
						...current,
						loading: false,
						error: cause instanceof Error ? cause.message : String(cause),
					}));
				}
			})();
		};
		refreshRef.current = refresh;
		setState({ ...EMPTY_STATE, loading: true });
		const unsubscribe = actions.events?.onEvent((event) => {
			if (event.type === "macro_execution_changed") {
				setState((current) => ({
					...current,
					macros: mergeMacroExecutions(current.macros, [event.execution]),
				}));
			} else if (event.type === "timecode_runtime_changed") {
				setState((current) => ({
					...current,
					timecodes: mergeTimecodes(current.timecodes, [event.snapshot]),
				}));
			}
		});
		void refresh();
		return () => {
			mounted = false;
			unsubscribe?.();
			if (refreshRef.current === refresh)
				refreshRef.current = async () => undefined;
		};
	}, [actions, enabled, showId]);

	const stopTimecode = useCallback(
		async (timecodeId: string) => {
			if (!showId || !actions) return false;
			const snapshot = await actions.timecodes.stop(showId, timecodeId);
			setState((current) => ({
				...current,
				timecodes: mergeTimecodes(current.timecodes, [snapshot]),
			}));
			if (!actions.events) await refreshRef.current();
			return true;
		},
		[actions, showId],
	);
	const cancelMacro = useCallback(
		async (executionId: string) => {
			if (!showId || !actions) return false;
			const execution = await actions.macros.cancel(showId, executionId);
			setState((current) => ({
				...current,
				macros: mergeMacroExecutions(current.macros, [execution]),
			}));
			if (!actions.events) await refreshRef.current();
			return true;
		},
		[actions, showId],
	);

	return { ...state, stopTimecode, cancelMacro };
}

function mergeTimecodes(
	current: readonly TimecodeRuntime[],
	incoming: readonly TimecodeRuntime[],
): TimecodeRuntime[] {
	const next = new Map(
		current.map((snapshot) => [snapshot.timecode_id, snapshot]),
	);
	for (const snapshot of incoming) {
		const previous = next.get(snapshot.timecode_id);
		if (!previous || snapshot.revision >= previous.revision) {
			next.set(snapshot.timecode_id, snapshot);
		}
	}
	return [...next.values()];
}

function mergeMacroExecutions(
	current: readonly MacroExecution[],
	incoming: readonly MacroExecution[],
): MacroExecution[] {
	const next = new Map(
		current.map((execution) => [execution.execution_id, execution]),
	);
	for (const execution of incoming) next.set(execution.execution_id, execution);
	return [...next.values()];
}
