import type { VisualizationSnapshot } from "../../api/types";
import { Stage3dCanvas } from "../Stage3dCanvas";
import { NativeStageSurface } from "./NativeStageSurface";
import { useNativeStagePane } from "./useNativeStagePane";
import { useStagePanePicks } from "./useStagePanePicks";
import { stageViewMode, useStagePanePicture } from "./useStagePanePicture";
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
	 * Every 3D view is the renderer's picture now — the full one and the
	 * lines-only one. It has drawn both all along, and having it draw them here is
	 * what lets the desk stop pushing live values into its web layer: nothing in the interface is
	 * rendering the rig any more, so nothing in the interface needs to be told what the rig is
	 * doing several dozen times a second.
	 *
	 * The desk's own drawing stays as the fallback, for a browser, a platform that cannot share a
	 * picture between processes, or an installation missing its renderer.
	 */
	/*
	 * Follow Preload is drawn here too. The desk has no preload output to decode, so the renderer
	 * takes the live rig from the desk's universes and lays the preload's own static values over
	 * the fixtures that have them: a fixture with nothing preloaded goes on showing what it is
	 * doing, and one that is preloaded shows what it is about to do, in every attribute the
	 * preload names — where it will point included.
	 *
	 * Dynamics are not applied, so a preloaded dynamic shows its fixture's live state. That is the
	 * honest answer: a dynamic is a running function rather than a value, and reproducing one here
	 * would be a second implementation of something the desk already owns.
	 */
	const wantsNative = true;
	const nativePane = useNativeStagePane(wantsNative);
	/*
	 * The renderer resolves what is under the pointer; this decides what that means. Selection is
	 * the desk's, and a renderer holding its own idea of it would be a second answer to the one
	 * question an operator has to be able to trust — so what crosses back is a fixture, and the
	 * gesture applied to it is the same one the desk's own Stage applies.
	 */
	useStagePanePicks(nativePane, selection, interactive);
	// The picture settings cross when the renderer starts, not only when one is moved.
	useStagePanePicture(
		nativePane,
		// The plan projections belong to the 2D Stage, which is a different component and still the
		// desk's own drawing; this one is only ever asked for a 3D view.
		stageViewMode(options.view, options.renderQuality, ""),
		options.followPreload,
	);
	return (
		<div
			className="stage-canvas stage-canvas-3d"
			data-stage-render-quality={options.renderQuality}
			data-stage-renderer={nativePane.active ? "native" : "web"}
		>
			{/*
			 * Mounted as soon as the view asks for the renderer, not once the renderer answers.
			 * This element is what reports where the pane is, and the desk cannot start a renderer
			 * for a rectangle nobody has measured — waiting for `active` to mount it meant waiting
			 * for something that could then never happen.
			 */}
			{wantsNative ? (
				<NativeStageSurface pane={nativePane} interactive={interactive} />
			) : null}
			{wantsNative && nativePane.active ? null : (
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
