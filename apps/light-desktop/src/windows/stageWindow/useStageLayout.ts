import {
	useStagePositions,
	useStagePositions2dConfig,
	useStagePositions3d,
} from "../../features/stageLayout/StageLayoutState";
import type { StageLayoutModel } from "./types";

/** Read-only stage layout for rendering; positions are edited in the Show Patch. */
export function useStageLayout(): StageLayoutModel {
	return {
		positions: useStagePositions(),
		positions3d: useStagePositions3d(),
		positions2dConfig: useStagePositions2dConfig(),
	};
}
