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
	highlightFixtures = [],
	camera3d,
	pixelRatioCap,
	selection,
	active,
	paneId,
	interactive = true,
}: {
	fixtures: Stage3dFixture[];
	visualization: VisualizationSnapshot | null;
	options: StageOptionsModel;
	patchSelectionPreview: boolean;
	patchPreviewFixtures: string[];
	highlightFixtures?: string[];
	camera3d: StageWindowProps["camera3d"];
	pixelRatioCap?: number;
	selection: StageSelectionModel;
	active?: boolean;
	paneId?: string;
	interactive?: boolean;
}) {
	return (
		<div
			className="stage-canvas stage-canvas-3d"
			data-stage-render-quality={options.renderQuality}
		>
			<Stage3dCanvas
				fixtures={fixtures}
				visualization={visualization}
				selected={selection.fixtureIds}
				virtualHighlight={[
					...new Set([
						...highlightFixtures,
						...(patchSelectionPreview ? patchPreviewFixtures : []),
					]),
				]}
				showSelection={options.showSelection}
				showFloorGrid={options.showFloorGrid}
				showBeamGuides={options.showBeamGuides}
				renderQuality={options.renderQuality}
				environmentBrightness={options.environmentBrightness}
				camera3d={camera3d}
				pixelRatioCap={pixelRatioCap}
				visualizationLane={options.followPreload ? "preload" : "normal"}
				visualizationActive={active ?? false}
				paneId={paneId}
				onSelect={(fixtureId, additive) => {
					if (!interactive) return;
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
