import type { VisualizationSnapshot } from "../../api/types";
import { Stage3dCanvas } from "../Stage3dCanvas";
import type { Stage3dFixture } from "../stage3dScene";
import type { StageOptionsModel, StageWindowProps } from "./types";
import type { StageSelectionModel } from "./useStageSelection";

export function Stage3dView({
	fixtures,
	visualization,
	options,
	patchSelectionPreview,
	patchPreviewFixtures,
	camera3d,
	selection,
}: {
	fixtures: Stage3dFixture[];
	visualization: VisualizationSnapshot | null;
	options: StageOptionsModel;
	patchSelectionPreview: boolean;
	patchPreviewFixtures: string[];
	camera3d: StageWindowProps["camera3d"];
	selection: StageSelectionModel;
}) {
	return (
		<div className="stage-canvas stage-canvas-3d">
			<Stage3dCanvas
				fixtures={fixtures}
				visualization={visualization}
				selected={selection.fixtureIds}
				virtualHighlight={patchSelectionPreview ? patchPreviewFixtures : []}
				showSelection={options.showSelection}
				showFloorGrid={options.showFloorGrid}
				showBeamGuides={options.showBeamGuides}
				renderQuality={options.renderQuality}
				environmentBrightness={options.environmentBrightness}
				camera3d={camera3d}
				onSelect={(fixtureId, additive) => {
					void selection.applyFixtureGesture(
						fixtureId,
						additive && selection.fixtureIdSet.has(fixtureId)
							? "remove"
							: "add",
					);
				}}
			/>
		</div>
	);
}
