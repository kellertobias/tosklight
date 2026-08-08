import { NativeStageSurface } from "./NativeStageSurface";
import { useNativeStagePane } from "./useNativeStagePane";
import { useStagePanePicks } from "./useStagePanePicks";
import { useStagePaneSelection } from "./useStagePaneSelection";
import { stageViewMode, useStagePanePicture } from "./useStagePanePicture";
import type { StageOptionsModel } from "./types";
import type { StageSelectionModel } from "./useStageSelection";

/**
 * The Stage, drawn by the ToskLight renderer.
 *
 * Every view is drawn here — the 2D plan, the 3D outline view and the 3D Viz picture — and the
 * desk draws none of them. It runs beside the desk rather than inside it, so a graphics driver
 * that takes the renderer down takes only the picture with it: the Programmer, playback and DMX
 * output are untouched, and the Stage comes back on its own.
 *
 * There is no second drawing behind this one. A Stage the renderer cannot draw says so, which is
 * the honest answer: a desk that quietly substituted a different picture of the same rig would be
 * showing an operator something they did not ask for and cannot tell apart from what they did.
 *
 * Follow Preload is drawn here too. The renderer takes the live rig from the desk's universes and
 * lays the preload's own static values over the fixtures that have them, so a fixture with nothing
 * preloaded goes on showing what it is doing and one that is preloaded shows what it is about to
 * do, in every attribute the preload names — where it will point included. Dynamics are not
 * applied: a dynamic is a running function rather than a value, and reproducing one here would be
 * a second implementation of something the desk already owns.
 */
export function StageRendererView({
	options,
	selection,
	active,
	interactive = true,
}: {
	options: StageOptionsModel;
	highlightFixtures?: string[];
	selection: StageSelectionModel;
	active?: boolean;
	interactive?: boolean;
}) {
	const pane = useNativeStagePane(true);
	/*
	 * The renderer resolves what is under the pointer; this decides what that means. Selection is
	 * the desk's, and a renderer holding its own idea of it would be a second answer to the one
	 * question an operator has to be able to trust — so what crosses back is a fixture, and the
	 * gesture applied to it is the same one every other selection surface applies.
	 */
	useStagePanePicks(pane, selection, interactive);
	// And the answer goes back, so the renderer can draw what the desk decided was selected.
	useStagePaneSelection(pane, selection);
	// The picture settings cross when the renderer starts, not only when one is moved.
	useStagePanePicture(
		pane,
		stageViewMode(options.view, options.side2d),
		options.followPreload,
	);
	return (
		<div
			className="stage-canvas stage-canvas-3d"
			data-stage-view={options.view}
			data-stage-renderer={pane.active ? "native" : "unavailable"}
		>
			{/*
			 * Mounted as soon as the pane exists, not once the renderer answers. This element is
			 * what reports where the pane is, and the desk cannot start a renderer for a rectangle
			 * nobody has measured — waiting for the renderer to mount it would be waiting for
			 * something that could then never happen.
			 */}
			<NativeStageSurface pane={pane} interactive={interactive} />
			{active && !pane.active && (
				<div className="stage-unavailable" role="status">
					<strong>No Stage on this screen</strong>
					<span>
						{pane.trouble ??
							"This screen cannot draw a Stage. The rig, the Programmer and DMX output are unaffected."}
					</span>
				</div>
			)}
		</div>
	);
}
