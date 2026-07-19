import { ParameterControlView } from "./parameterControls/ParameterControlView";
import { useParameterController } from "./parameterControls/useParameterController";
import { StageCommandControls } from "./StageCommandControls";
import { useApp } from "../../state/AppContext";

export {
	type DirectControlChoice,
	type DirectValueChoice,
	directProgrammerChoices,
} from "./parameterControls/model";

export function ParameterControls({ active = true }: { active?: boolean } = {}) {
	const { state } = useApp();
	const stageVisible =
		state.builtIn === "stage" ||
		state.desks
			.find((desk) => desk.id === state.activeDeskId)
			?.panes.some((pane) => pane.kind === "stage");
	if (state.stageMode !== "select" && stageVisible)
		return <StageCommandControls />;
	return <ProgrammerParameterControls active={active} />;
}

function ProgrammerParameterControls({ active }: { active: boolean }) {
	const controller = useParameterController(active);
	return <ParameterControlView controller={controller} />;
}
