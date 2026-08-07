import { useEffect } from "react";
import { useDesktopBridge } from "../../platform/desktop";
import { useApp } from "../../state/AppContext";
import type { NativeStagePane } from "./useNativeStagePane";

/**
 * Keeping the renderer's picture settings in step with the desk's.
 *
 * These belong to the renderer — it is drawing the picture — so they have to cross to it, and they
 * have to cross when it starts as well as when they change. A renderer sent nothing until the first
 * time an operator moved a control draws with its own defaults until then, which is a Stage that
 * looks wrong until you touch a setting and then suddenly corrects itself.
 *
 * Sent whole rather than per control, so a renderer is never left holding a mixture of what the
 * operator chose and what it started with.
 */
/**
 * Which way the renderer is asked to look, from the view the operator chose.
 *
 * The renderer has drawn every one of these all along — the plan projections, a lines-only 3D and
 * a full one — so a Stage in any view can be its picture rather than the desk's. That is what lets
 * the web layer stop being sent live values at all: it is no longer drawing the rig in any mode.
 */
export function stageViewMode(
	view: string,
	renderQuality: string,
	projection: string,
): string {
	if (view === "2d") {
		return (
			{
				top_to_bottom: "top_down",
				bottom_to_top: "top_down",
				left_to_right: "left_to_right",
				right_to_left: "right_to_left",
				front_to_back: "front_to_back",
				back_to_front: "back_to_front",
			}[projection] ?? "top_down"
		);
	}
	// Lines only is a way of looking at the rig rather than a quality setting: it draws where the
	// light goes without pretending to show what it looks like.
	if (renderQuality === "none" || renderQuality === "lines_only") return "lines_3d";
	if (renderQuality === "lines_and_beams") return "simple_3d";
	return "full_3d";
}

export function useStagePanePicture(
	pane: NativeStagePane,
	mode = "full_3d",
	followPreload = false,
) {
	const bridge = useDesktopBridge();
	const { state } = useApp();
	const active = pane.active;
	const {
		stageVizAtmosphere,
		stageEnvironmentBrightness,
		stageVizQuality,
		stageVizExposure,
		stageVizLaserBrightness,
		stageVizShowLabels,
	} = state;

	useEffect(() => {
		if (!active) return;
		void bridge.setStagePanePicture({
			atmosphere: stageVizAtmosphere,
			ambient: stageEnvironmentBrightness,
			quality: stageVizQuality,
			exposure: stageVizExposure,
			laserBrightness: stageVizLaserBrightness,
			showLabels: stageVizShowLabels,
			mode,
			followPreload,
		});
	}, [
		active,
		bridge,
		mode,
		followPreload,
		stageVizAtmosphere,
		stageEnvironmentBrightness,
		stageVizQuality,
		stageVizExposure,
		stageVizLaserBrightness,
		stageVizShowLabels,
	]);
}
