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
	 * The native renderer draws this pane where the desk can run it: a desktop window with a
	 * surface underneath the interface, a renderer beside the application, and a way to move a
	 * picture between the two processes. Anywhere else — a browser, a platform without a shared
	 * surface, an installation missing its renderer — the web renderer below draws the same Stage,
	 * which is why this is a swap rather than a requirement.
	 */
	const nativePane = useNativeStagePane();
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
