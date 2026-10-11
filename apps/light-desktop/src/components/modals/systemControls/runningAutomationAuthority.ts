import { useMemo } from "react";
import { buildRunningRows } from "../../../features/running/model";
import { useRunningRuntimeActions } from "../../../features/running/RunningRuntimeActionsContext";
import { useRunningSupplementalAuthority } from "../../../features/running/useRunningSupplementalAuthority";

/** Uses the same scoped runtime authority as the independently stoppable Running pane. */
export function useRunningAutomationAuthority(enabled: boolean, showId: string | null) {
	const actions = useRunningRuntimeActions();
	const supplemental = useRunningSupplementalAuthority(enabled, showId, actions);
	const rows = useMemo(() => buildRunningRows({
		playbacks: [],
		dynamics: [],
		timecodes: supplemental.timecodes,
		timecodeDefinitions: supplemental.timecodeDefinitions,
		macros: supplemental.macros,
		releasePlayback: async () => false,
		turnOffDynamic: async () => false,
		stopTimecode: supplemental.stopTimecode,
		cancelMacro: supplemental.cancelMacro,
	}), [supplemental]);
	return {
		rows,
		loading: supplemental.loading,
		error: supplemental.error,
		ready: actions !== null && showId !== null && !supplemental.loading && !supplemental.error,
	};
}
