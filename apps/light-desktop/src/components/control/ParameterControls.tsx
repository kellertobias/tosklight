import { useHardwareConnected } from "../../features/deskSnapshot/DeskSnapshotState";
import { useApp } from "../../state/AppContext";
import { ParameterControlView } from "./parameterControls/ParameterControlView";
import { useParameterController } from "./parameterControls/useParameterController";
import { StageCommandControls } from "./StageCommandControls";
import { StageVizCameraControls } from "./StageVizCameraControls";

export function ParameterControls({
	active = true,
}: {
	active?: boolean;
} = {}) {
	const { state } = useApp();
	const hardwareConnected = Boolean(
		useHardwareConnected() || state.midiProfile,
	);
	const stageVisible =
		state.builtIn === "stage" ||
		state.desks
			.find((desk) => desk.id === state.activeDeskId)
			?.panes.some((pane) => pane.kind === "stage");
	if (state.stageMode !== "select" && stageVisible) {
		// A Stage drawn by the renderer has a camera in the world rather than a pan and a zoom over
		// a drawing, so Navigate addresses that camera directly: where it stands, and where it
		// looks. The desk's own Stage keeps the controls that match how it draws.
		const drawnByTheRenderer =
			state.builtIn === "stage"
				? state.stageView === "3d-viz"
				: state.desks
						.find((desk) => desk.id === state.activeDeskId)
						?.panes.some(
							(pane) =>
								pane.kind === "stage" &&
								(pane.stageView ?? state.stageView) === "3d-viz",
						);
		return drawnByTheRenderer ? (
			<StageVizCameraControls hardwareConnected={hardwareConnected} />
		) : (
			<StageCommandControls />
		);
	}
	return <ProgrammerParameterControls active={active} />;
}

function ProgrammerParameterControls({ active }: { active: boolean }) {
	const controller = useParameterController(active);
	return <ParameterControlView controller={controller} />;
}
