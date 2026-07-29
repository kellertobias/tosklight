import { type Dispatch, type MutableRefObject, useEffect, useRef } from "react";
import * as THREE from "three";
import type { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { useDesktopVisualizationRuntimeRenderAcknowledgement } from "../../features/visualizationRuntime/VisualizationRuntimeView";
import type { Action } from "../../state/appReducer";
import { mountStageRenderer } from "./rendererLifecycle";
import type { StageSceneController } from "./useStageScene";

export type StageCamera = {
	position: readonly [number, number, number];
	target: readonly [number, number, number];
};

export type StageNavigation = {
	zoom: number;
	orbitX: number;
	orbitY: number;
};

export function useStageRenderer({
	hostRef,
	controller,
	dispatch,
	diagnosticsRef,
}: {
	hostRef: MutableRefObject<HTMLDivElement | null>;
	controller: StageSceneController;
	dispatch: Dispatch<Action>;
	diagnosticsRef: MutableRefObject<{
		lane: "normal" | "preload";
		paneId: string | null;
	}>;
}) {
	const acknowledgeDesktopMirrorRender =
		useDesktopVisualizationRuntimeRenderAcknowledgement();
	const cameraRef = useRef<THREE.PerspectiveCamera | null>(null);
	const controlsRef = useRef<OrbitControls | null>(null);
	const cameraTargetRef = useRef(new THREE.Vector3(0, 1.8, -4));

	useEffect(() => {
		const container = hostRef.current;
		if (!container) return;
		return mountStageRenderer({
			container,
			controller,
			dispatch,
			diagnosticsRef,
			cameraRef,
			controlsRef,
			cameraTargetRef,
			acknowledgeDesktopMirrorRender,
		});
	}, [
		acknowledgeDesktopMirrorRender,
		controller,
		diagnosticsRef,
		dispatch,
		hostRef,
	]);

	return { cameraRef, controlsRef, cameraTargetRef };
}

export function useStageCamera({
	camera,
	controls,
	cameraTarget,
	resolvedCamera,
	navigation,
}: {
	camera: MutableRefObject<THREE.PerspectiveCamera | null>;
	controls: MutableRefObject<OrbitControls | null>;
	cameraTarget: MutableRefObject<THREE.Vector3>;
	resolvedCamera: StageCamera | undefined;
	navigation: StageNavigation;
}) {
	useEffect(() => {
		const activeCamera = camera.current;
		const activeControls = controls.current;
		if (!activeCamera || !activeControls) return;
		if (resolvedCamera) {
			activeCamera.position.set(...resolvedCamera.position);
			activeControls.target.set(...resolvedCamera.target);
		} else {
			const orbitRadius = Math.max(2, 12 / Math.max(0.2, navigation.zoom));
			const azimuth = THREE.MathUtils.degToRad(navigation.orbitX);
			const elevation = THREE.MathUtils.degToRad(18 + navigation.orbitY);
			activeCamera.position.set(
				Math.sin(azimuth) * orbitRadius,
				1.8 + Math.sin(elevation) * orbitRadius,
				-4 + Math.cos(azimuth) * Math.cos(elevation) * orbitRadius,
			);
			activeControls.target.copy(cameraTarget.current);
		}
		activeControls.update();
		// OrbitControls emits "change" when this alters the camera, which invalidates
		// the demand-driven renderer through the listener installed above.
	}, [
		camera,
		cameraTarget,
		controls,
		navigation.orbitX,
		navigation.orbitY,
		navigation.zoom,
		resolvedCamera,
	]);
}
