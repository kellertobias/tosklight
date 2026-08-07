import type { VisualizationSnapshot } from "../../api/types";
import { Stage3dCanvas } from "../Stage3dCanvas";
import { NativeStageSurface } from "./NativeStageSurface";
import { useNativeStagePane } from "./useNativeStagePane";
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
	/*
	 * The operator chooses which renderer draws this, rather than the desk taking the better one
	 * whenever it can: 3D is the desk's own picture and 3D Viz is the renderer's. The choice is
	 * still only offered where the renderer can run, and if it cannot start after all, the pane
	 * falls back to the desk's own drawing rather than showing nothing.
	 */
	const nativePane = useNativeStagePane(options.view === "3d-viz");
	return (
		<div
			className="stage-canvas stage-canvas-3d"
			data-stage-render-quality={options.renderQuality}
			data-stage-renderer={nativePane.active ? "native" : "web"}
		>
			{nativePane.active ? (
				<NativeStageSurface pane={nativePane} interactive={interactive} />
			) : null}
			{nativePane.active ? null : (
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
			)}
		</div>
	);
}
