import { type Dispatch, type MutableRefObject, useEffect, useRef } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { frontendPerformanceDiagnostics } from "../../features/frontendWarmup/diagnostics";
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
		});
		frontendPerformanceDiagnostics.recordStageRendererCreated();
		renderer.setPixelRatio(Math.min(devicePixelRatio, 1.25));
		renderer.outputColorSpace = THREE.SRGBColorSpace;
		container.replaceChildren(renderer.domElement);
		const camera = new THREE.PerspectiveCamera(48, 1, 0.1, 100);
		const controls = new OrbitControls(camera, renderer.domElement);
		controls.enableDamping = true;
		cameraRef.current = camera;
		controlsRef.current = controls;
		let frame: number | null = null;
		let contextLost = false;
		const render = () => {
			frame = null;
			if (contextLost) return;
			frontendPerformanceDiagnostics.recordStageRafCallback();
			const controlsChanged = controls.update();
			if (controller.sceneRef.current) {
				const startedAt = performance.now();
				renderer.render(controller.sceneRef.current, camera);
				const submittedAt = performance.now();
				frontendPerformanceDiagnostics.recordStageRender({
					submittedAt,
					durationMs: submittedAt - startedAt,
					calls: renderer.info.render.calls,
					triangles: renderer.info.render.triangles,
					lines: renderer.info.render.lines,
					points: renderer.info.render.points,
					geometries: renderer.info.memory.geometries,
					textures: renderer.info.memory.textures,
				});
				frontendPerformanceDiagnostics.recordStageFrameCanvasSubmitted(
					controller.displayedVisualizationRef.current?.generated_at,
					controller.visualizationSettledRef.current,
					controller.displayedVisualizationRef.current?.preload
						? "preload"
						: "normal",
				);
			}
			if (controlsChanged && frame === null)
				frame = requestAnimationFrame(render);
		};
		const requestRender = () => {
			if (!contextLost && frame === null) frame = requestAnimationFrame(render);
		};
		const handleContextLost = (event: Event) => {
			event.preventDefault();
			contextLost = true;
			if (frame !== null) cancelAnimationFrame(frame);
			frame = null;
		};
		const handleContextRestored = () => {
			contextLost = false;
			renderer.resetState();
			requestRender();
		};
		renderer.domElement.addEventListener("webglcontextlost", handleContextLost);
		renderer.domElement.addEventListener(
			"webglcontextrestored",
			handleContextRestored,
		);
		const rememberCamera = () => {
			cameraTargetRef.current.copy(controls.target);
			requestRender();
		};
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
		controller.invalidateRef.current = requestRender;
		const unbindPointer = bindStagePointerInteraction(
			renderer,
			camera,
			controller,
		);
		const resize = () => {
			const { width, height } = container.getBoundingClientRect();
			renderer.setSize(width, height, false);
			camera.aspect = width / Math.max(height, 1);
			camera.updateProjectionMatrix();
			requestRender();
		};
		const observer = new ResizeObserver(resize);
		observer.observe(container);
		resize();
		requestRender();
		return () => {
			if (frame !== null) cancelAnimationFrame(frame);
			if (controller.invalidateRef.current === requestRender)
				controller.invalidateRef.current = null;
			observer.disconnect();
			controls.dispose();
			controls.removeEventListener("change", rememberCamera);
			controls.removeEventListener("end", publishCamera);
			unbindPointer();
			renderer.domElement.removeEventListener(
				"webglcontextlost",
				handleContextLost,
			);
			renderer.domElement.removeEventListener(
				"webglcontextrestored",
				handleContextRestored,
			);
			const scene = controller.sceneRef.current;
			controller.sceneRef.current = null;
			controller.fixtureObjectsRef.current = new Map();
			if (scene) disposeScene(scene);
			cameraRef.current = null;
			controlsRef.current = null;
			renderer.forceContextLoss();
			renderer.dispose();
			frontendPerformanceDiagnostics.recordStageRendererDisposed();
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
