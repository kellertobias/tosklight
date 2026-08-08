import { useEffect } from "react";
import { useDesktopBridge } from "../../platform/desktop";
import { useApp } from "../../state/AppContext";
import type { Stage2dSide, StageView } from "../../types";
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
 * Every Stage is the renderer's picture, so this is the whole mapping: a 2D Stage is one of its
 * orthographic plans, chosen by which side the operator is looking from; a 3D Stage is its
 * outline view; a 3D Viz Stage is the full one. Nothing else decides it — in particular no
 * quality setting does, because how much a beam costs to draw is not a way of looking at a rig.
 */
export function stageViewMode(view: StageView, side: Stage2dSide): string {
	if (view === "2d") {
		return {
			top: "top_down",
			front: "front_to_back",
			back: "back_to_front",
			left: "left_to_right",
			right: "right_to_left",
		}[side];
	}
	return view === "3d" ? "lines_3d" : "full_3d";
}

/** `#rrggbb` as the three linear components the renderer wants. */
export function backgroundColour(hex: string): [number, number, number] {
	const match = /^#?([0-9a-f]{6})$/i.exec(hex.trim());
	if (!match) return [0.008, 0.01, 0.016];
	const value = Number.parseInt(match[1], 16);
	// The operator picked the colour in a picker, which works in the sRGB the screen shows; the
	// renderer clears to a linear value. Converting rather than passing it straight through is
	// what makes a chosen colour arrive as the colour that was chosen.
	const channel = (byte: number) => {
		const scaled = byte / 255;
		return scaled <= 0.04045
			? scaled / 12.92
			: ((scaled + 0.055) / 1.055) ** 2.4;
	};
	return [
		channel((value >> 16) & 0xff),
		channel((value >> 8) & 0xff),
		channel(value & 0xff),
	];
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
		stageShowFloorGrid,
		stageVizBackground,
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
			floorGrid: stageShowFloorGrid,
			background: backgroundColour(stageVizBackground),
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
		stageShowFloorGrid,
		stageVizBackground,
	]);
}
