import { useMemo } from "react";
import { createLightApi } from "../api/client/api";
import { MacrosApiClient } from "../api/client/macros";
import { useRunningDynamicsAuthority } from "../components/modals/systemControls/runningDynamicsAuthority";
import { useRunningPlaybackAuthority } from "../components/modals/systemControls/runningPlaybackAuthority";
import { useActiveShowId } from "../features/deskSnapshot/DeskSnapshotState";
import { useDynamicsActions } from "../features/dynamics/DynamicsActionsContext";
import { buildRunningRows } from "../features/running/model";
import { RunningPane } from "../features/running/RunningPane";
import {
	RunningRuntimeActionsProvider,
	TimecodeRunningApiClient,
	useRunningRuntimeActions,
} from "../features/running/RunningRuntimeActionsContext";
import { useRunningSupplementalAuthority } from "../features/running/useRunningSupplementalAuthority";
import type { WindowProps } from "./windowTypes";

export function RunningWindow({
	active = true,
	compact = false,
	runningFilter = "all",
	onRunningFilterChange,
}: WindowProps) {
	const configuredActions = useRunningRuntimeActions();
	const fallback = useMemo(() => {
		const api = createLightApi();
		const transport = api.runtime.capabilityTransport();
		return {
			macros: new MacrosApiClient(transport),
			timecodes: new TimecodeRunningApiClient(transport),
			showObjects: api.showObjects,
		};
	}, []);
	return (
		<RunningRuntimeActionsProvider actions={configuredActions ?? fallback}>
			<RunningWindowAuthority
				active={active}
				compact={compact}
				runningFilter={runningFilter}
				onRunningFilterChange={onRunningFilterChange}
			/>
		</RunningRuntimeActionsProvider>
	);
}

function RunningWindowAuthority({
	active,
	compact,
	runningFilter,
	onRunningFilterChange,
}: Required<Pick<WindowProps, "active" | "compact" | "runningFilter">> &
	Pick<WindowProps, "onRunningFilterChange">) {
	const showId = useActiveShowId();
	const dynamicsActions = useDynamicsActions();
	const runtimeActions = useRunningRuntimeActions();
	const playbacks = useRunningPlaybackAuthority(active);
	const dynamics = useRunningDynamicsAuthority(
		active,
		showId,
		dynamicsActions?.dynamics ?? null,
		dynamicsActions?.events ?? null,
	);
	const supplemental = useRunningSupplementalAuthority(
		active,
		showId,
		runtimeActions,
	);
	const rows = useMemo(
		() =>
			buildRunningRows({
				playbacks: playbacks.sources,
				dynamics: dynamics.rows,
				timecodes: supplemental.timecodes,
				timecodeDefinitions: supplemental.timecodeDefinitions,
				macros: supplemental.macros,
				releasePlayback: (source) => playbacks.release(source),
				turnOffDynamic: (controller) => dynamics.off(controller),
				stopTimecode: supplemental.stopTimecode,
				cancelMacro: supplemental.cancelMacro,
			}),
		[dynamics, playbacks, supplemental],
	);
	return (
		<RunningPane
			rows={rows}
			loading={playbacks.loading || dynamics.loading || supplemental.loading}
			error={dynamics.error ?? supplemental.error}
			compact={compact}
			filter={runningFilter}
			onFilterChange={onRunningFilterChange}
		/>
	);
}
