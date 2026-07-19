import { type Dispatch, type MutableRefObject, useEffect, useRef } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import type { Action } from "../../state/appReducer";
import { disposeScene } from "../stage3dScene";
import { bindStagePointerInteraction } from "./pointerInteraction";
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
}: {
	hostRef: MutableRefObject<HTMLDivElement | null>;
	controller: StageSceneController;
	dispatch: Dispatch<Action>;
}) {
	const cameraRef = useRef<THREE.PerspectiveCamera | null>(null);
	const controlsRef = useRef<OrbitControls | null>(null);
	const cameraTargetRef = useRef(new THREE.Vector3(0, 1.8, -4));

	useEffect(() => {
		const container = hostRef.current;
		if (!container) return;
		const renderer = new THREE.WebGLRenderer({
			antialias: true,
			preserveDrawingBuffer: true,
		});
		renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
		renderer.outputColorSpace = THREE.SRGBColorSpace;
		container.replaceChildren(renderer.domElement);
		const camera = new THREE.PerspectiveCamera(48, 1, 0.1, 100);
		const controls = new OrbitControls(camera, renderer.domElement);
		controls.enableDamping = true;
		cameraRef.current = camera;
		controlsRef.current = controls;
		const rememberCamera = () => cameraTargetRef.current.copy(controls.target);
		const publishCamera = () => {
			const offset = camera.position.clone().sub(controls.target);
			dispatch({
				type: "SET_STAGE_NAVIGATION",
				zoom: 12 / Math.max(2, offset.length()),
				orbitX: THREE.MathUtils.radToDeg(Math.atan2(offset.x, offset.z)),
				orbitY:
					THREE.MathUtils.radToDeg(Math.asin(offset.y / offset.length())) - 18,
			});
		};
		controls.addEventListener("change", rememberCamera);
		controls.addEventListener("end", publishCamera);
		const unbindPointer = bindStagePointerInteraction(
			renderer,
			camera,
			controls,
			controller,
		);
		const resize = () => {
			const { width, height } = container.getBoundingClientRect();
			renderer.setSize(width, height, false);
			camera.aspect = width / Math.max(height, 1);
			camera.updateProjectionMatrix();
		};
		const observer = new ResizeObserver(resize);
		observer.observe(container);
		resize();
		let frame = 0;
		const animate = () => {
			controls.update();
			if (controller.sceneRef.current)
				renderer.render(controller.sceneRef.current, camera);
			frame = requestAnimationFrame(animate);
		};
		animate();
		return () => {
			cancelAnimationFrame(frame);
			observer.disconnect();
			controls.dispose();
			controls.removeEventListener("change", rememberCamera);
			controls.removeEventListener("end", publishCamera);
			unbindPointer();
			const scene = controller.sceneRef.current;
			controller.sceneRef.current = null;
			controller.fixtureObjectsRef.current = new Map();
			if (scene) disposeScene(scene);
			cameraRef.current = null;
			controlsRef.current = null;
			renderer.forceContextLoss();
			renderer.dispose();
		};
	}, [controller, dispatch, hostRef]);

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
