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
export function useStagePanePicture(pane: NativeStagePane) {
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
		});
	}, [
		active,
		bridge,
		stageVizAtmosphere,
		stageEnvironmentBrightness,
		stageVizQuality,
		stageVizExposure,
		stageVizLaserBrightness,
		stageVizShowLabels,
	]);
}
