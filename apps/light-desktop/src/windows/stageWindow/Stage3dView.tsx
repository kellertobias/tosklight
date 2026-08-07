import type { VisualizationSnapshot } from "../../api/types";
import { Stage3dCanvas } from "../Stage3dCanvas";
import { NativeStageSurface } from "./NativeStageSurface";
import { useNativeStagePane } from "./useNativeStagePane";
import { useStagePanePicks } from "./useStagePanePicks";
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
	/*
	 * Follow Preload stays with the desk's own renderer, because the native one cannot draw it
	 * honestly: the desk has no preload output. Its live values are universes carrying every
	 * attribute, but the preload lane exists only as a colour-and-intensity projection, so a
	 * moving head would hold its live angle while its colour changed — a picture that looks
	 * right and is not. Better the pane a Stage already has than a preload nobody can trust.
	 */
	const wantsNative = options.view === "3d-viz" && !options.followPreload;
	const nativePane = useNativeStagePane(wantsNative);
	/*
	 * The renderer resolves what is under the pointer; this decides what that means. Selection is
	 * the desk's, and a renderer holding its own idea of it would be a second answer to the one
	 * question an operator has to be able to trust — so what crosses back is a fixture, and the
	 * gesture applied to it is the same one the desk's own Stage applies.
	 */
	useStagePanePicks(nativePane, selection, interactive);
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
