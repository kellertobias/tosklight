import { useCallback, useEffect, useRef, useState } from "react";
import type {
	MacroExecutionSnapshot,
	TimecodeDefinition,
	TimecodeTransportSnapshot,
} from "../../api/generated/light-wire";
import type { VersionedObject } from "../../api/types";
import type { RunningRuntimeActions } from "./RunningRuntimeActionsContext";

interface RunningSupplementalState {
	loading: boolean;
	error: string | null;
	macros: MacroExecutionSnapshot[];
	timecodes: TimecodeTransportSnapshot[];
	timecodeDefinitions: VersionedObject<TimecodeDefinition>[];
}

const EMPTY_STATE: RunningSupplementalState = {
	loading: false,
	error: null,
	macros: [],
	timecodes: [],
	timecodeDefinitions: [],
};

/** Polls only runtimes that do not yet publish complete projection events. */
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
		let running: Promise<void> | null = null;
		let invalidated = false;
		const refresh = (): Promise<void> => {
			if (running) {
				invalidated = true;
				return running;
			}
			running = (async () => {
				do {
					invalidated = false;
					try {
						const [macroRuntime, timecodes, timecodeDefinitions] =
							await Promise.all([
								actions.macros.runtime(showId),
								actions.timecodes.runtime(showId),
								actions.showObjects.objects<TimecodeDefinition>(
									showId,
									"timecode",
								),
							]);
						if (!mounted) return;
						setState({
							loading: false,
							error: null,
							macros: macroRuntime.active,
							timecodes,
							timecodeDefinitions,
						});
					} catch (cause) {
						if (!mounted) return;
						setState((current) => ({
							...current,
							loading: false,
							error: cause instanceof Error ? cause.message : String(cause),
						}));
					}
				} while (mounted && invalidated);
			})().finally(() => {
				running = null;
			});
			return running;
		};
		refreshRef.current = refresh;
		setState({ ...EMPTY_STATE, loading: true });
		void refresh();
		const timer = window.setInterval(() => void refresh(), 500);
		return () => {
			mounted = false;
			window.clearInterval(timer);
			if (refreshRef.current === refresh)
				refreshRef.current = async () => undefined;
		};
	}, [actions, enabled, showId]);

	const stopTimecode = useCallback(
		async (timecodeId: string) => {
			if (!showId || !actions) return;
			await actions.timecodes.stop(showId, timecodeId);
			await refreshRef.current();
		},
		[actions, showId],
	);
	const cancelMacro = useCallback(
		async (executionId: string) => {
			if (!showId || !actions) return;
			await actions.macros.cancel(showId, executionId);
			await refreshRef.current();
		},
		[actions, showId],
	);

	return { ...state, stopTimecode, cancelMacro };
}
