import { useRef } from "react";
import type { VisualizationSnapshot } from "../api/types";
import { useApp } from "../state/AppContext";
import type { StageRenderQuality } from "../types";
import {
	type StageCamera,
	useStageCamera,
	useStageRenderer,
} from "./stage3d/useStageRenderer";
import { useStageScene } from "./stage3d/useStageScene";
import type { Stage3dFixture } from "./stage3dScene";

export const DEFAULT_STAGE_CAMERA_3D = {
	position: [0, 1.625, 8] as const,
	target: [0, 2.6, -4] as const,
};

interface Props {
	fixtures: Stage3dFixture[];
	visualization: VisualizationSnapshot | null;
	selected: readonly string[];
	virtualHighlight?: readonly string[];
	showSelection: boolean;
	showFloorGrid: boolean;
	showBeamGuides: boolean;
	renderQuality: StageRenderQuality;
	environmentBrightness: number;
	camera3d?: StageCamera;
	onSelect: (fixtureId: string, additive: boolean) => void;
}

export function Stage3dCanvas({
	fixtures,
	visualization,
	selected,
	virtualHighlight = [],
	showSelection,
	showFloorGrid,
	showBeamGuides,
	renderQuality,
	environmentBrightness,
	camera3d,
	onSelect,
}: Props) {
	const { state, dispatch } = useApp();
	const hostRef = useRef<HTMLDivElement>(null);
	const defaultCamera =
		state.stageZoom === 1 && state.stageOrbitX === 0 && state.stageOrbitY === 0
			? DEFAULT_STAGE_CAMERA_3D
			: undefined;
	const resolvedCamera = camera3d ?? defaultCamera;
	const controller = useStageScene({
		fixtures,
		visualization,
		selected,
		virtualHighlight,
		showSelection,
		showFloorGrid,
		showBeamGuides,
		renderQuality,
		environmentBrightness,
		callbacks: { onSelect },
	});
	const { cameraRef, controlsRef, cameraTargetRef } = useStageRenderer({
		hostRef,
		controller,
		dispatch,
	});
	useStageCamera({
		camera: cameraRef,
		controls: controlsRef,
		cameraTarget: cameraTargetRef,
		resolvedCamera,
		navigation: {
			zoom: state.stageZoom,
			orbitX: state.stageOrbitX,
			orbitY: state.stageOrbitY,
		},
	});
	return (
		<div
			className="stage-3d-canvas"
			data-camera-position={resolvedCamera?.position.join(",")}
			data-camera-target={resolvedCamera?.target.join(",")}
			data-environment-brightness={environmentBrightness}
			data-floor-grid={showFloorGrid ? "on" : "off"}
			data-beam-guides={showBeamGuides ? "on" : "off"}
			data-render-quality={renderQuality}
			ref={hostRef}
		/>
	);
}
