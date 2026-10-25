import { useEffect, useMemo } from "react";
import { useDesktopBridge } from "../../platform/desktop";
import { useApp } from "../../state/AppContext";
import type { Stage2dSide, StageView } from "../../types";
import type { StagePanePicture } from "../../platform/desktop/types";
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
 * outline view. Legacy full-render views use that same outline view in Control. The full
 * rendering controls remain in Architect.
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
	return "lines_3d";
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
	mode = "lines_3d",
	followPreload = false,
	paneId?: string,
	forcedShowBeamGuides?: boolean,
) {
	const bridge = useDesktopBridge();
	const { state } = useApp();
	const active = pane.active;
	const {
		stageEnvironmentBrightness,
		stageVizExposure,
		stageVizDetailedPeople,
		stageVizLaserBrightness,
		stageVizShowLabels,
		stageShowFloorGrid,
		stageShowBeamGuides,
		stageVizBackground,
	} = state;
	const paneSettings = paneId
		? state.desks
				.find((desk) => desk.id === state.activeDeskId)
				?.panes.find((candidate) => candidate.id === paneId)
		: undefined;
	const lampFogCloudiness = paneSettings?.lampFogCloudiness ?? 0.7;
	const lampFogTurbulence = paneSettings?.lampFogTurbulence ?? 1;
	const laserFogCloudiness = paneSettings?.laserFogCloudiness ?? 0;
	const laserFogTurbulence = paneSettings?.laserFogTurbulence ?? 0;
	const showBeamGuides = forcedShowBeamGuides ?? stageShowBeamGuides;

	const picture = useMemo<StagePanePicture>(() => ({
		atmosphere: 0,
		ambient: stageEnvironmentBrightness,
		quality: "draft",
		exposure: stageVizExposure,
		detailedPeople: stageVizDetailedPeople ?? false,
		laserBrightness: stageVizLaserBrightness,
		lampFogCloudiness,
		lampFogTurbulence,
		laserFogCloudiness,
		laserFogTurbulence,
		showLabels: stageVizShowLabels,
		floorGrid: stageShowFloorGrid,
		showBeamGuides,
		background: backgroundColour(stageVizBackground),
		mode: [
			"top_down",
			"left_to_right",
			"right_to_left",
			"front_to_back",
			"back_to_front",
		].includes(mode)
			? mode
			: "lines_3d",
		followPreload,
	}), [
		mode,
		followPreload,
		stageEnvironmentBrightness,
		stageVizExposure,
		stageVizDetailedPeople,
		stageVizLaserBrightness,
		lampFogCloudiness,
		lampFogTurbulence,
		laserFogCloudiness,
		laserFogTurbulence,
		stageVizShowLabels,
		stageShowFloorGrid,
		showBeamGuides,
		stageVizBackground,
	]);
	useEffect(() => {
		if (active) void Promise.resolve(bridge.setStagePanePicture(pane.id, picture)).catch((error) => pane.reportError?.(error));
	}, [active, bridge, pane.id, picture, pane.reportError]);
	return picture;
}
