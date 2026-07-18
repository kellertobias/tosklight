import { ParameterControlView } from "./parameterControls/ParameterControlView";
import { useParameterController } from "./parameterControls/useParameterController";
import { StageCommandControls } from "./StageCommandControls";

export {
	type DirectControlChoice,
	type DirectValueChoice,
	directProgrammerChoices,
} from "./parameterControls/model";

export function ParameterControls() {
	const controller = useParameterController();
	const { state } = controller;
	const stageVisible =
		state.builtIn === "stage" ||
		state.desks
			.find((desk) => desk.id === state.activeDeskId)
			?.panes.some((pane) => pane.kind === "stage");
	if (state.stageMode !== "select" && stageVisible)
		return <StageCommandControls />;
	return <ParameterControlView controller={controller} />;
}
