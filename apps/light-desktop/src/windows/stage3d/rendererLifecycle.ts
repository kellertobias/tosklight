import type { Dispatch, MutableRefObject } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { frontendPerformanceDiagnostics } from "../../features/frontendWarmup/diagnostics";
import type { Action } from "../../state/appReducer";
import { disposeScene } from "../stage3dScene";
import { bindStagePointerInteraction } from "./pointerInteraction";
import { rendererCapabilities } from "./rendererDiagnostics";
import { createStageRenderLoop } from "./rendererRenderLoop";
import type { StageSceneController } from "./sceneTypes";

type RendererLifecycleOptions = {
	container: HTMLDivElement;
	controller: StageSceneController;
	dispatch: Dispatch<Action>;
	diagnosticsRef: MutableRefObject<{
		lane: "normal" | "preload";
		paneId: string | null;
	}>;
	cameraRef: MutableRefObject<THREE.PerspectiveCamera | null>;
	controlsRef: MutableRefObject<OrbitControls | null>;
	cameraTargetRef: MutableRefObject<THREE.Vector3>;
	acknowledgeDesktopMirrorRender: (() => void) | null | undefined;
	pixelRatioCap?: number;
};

export function mountStageRenderer(options: RendererLifecycleOptions) {
	let disposed = false;
	let cleanup: ((disposeRetainedScene: boolean) => void) | null = null;
	const mount = (recoveringFromContextLoss: boolean) => {
		if (disposed) return;
		const retainedCameraPosition =
			options.cameraRef.current?.position.clone() ?? null;
		const retainedCameraTarget =
			options.controlsRef.current?.target.clone() ??
			options.cameraTargetRef.current.clone();
		cleanup?.(false);
		cleanup = mountStageRendererInstance(
			options,
			mount,
			recoveringFromContextLoss,
			retainedCameraPosition,
			retainedCameraTarget,
		);
	};
	mount(false);
	return () => {
		disposed = true;
		cleanup?.(true);
		cleanup = null;
	};
}

function mountStageRendererInstance(
	{
		container,
		controller,
		dispatch,
		diagnosticsRef,
		cameraRef,
		controlsRef,
		cameraTargetRef,
		acknowledgeDesktopMirrorRender,
		pixelRatioCap,
	}: RendererLifecycleOptions,
	remountAfterContextLoss: (recoveringFromContextLoss: boolean) => void,
	recoveringFromContextLoss: boolean,
	retainedCameraPosition: THREE.Vector3 | null,
	retainedCameraTarget: THREE.Vector3,
) {
	const renderer = new THREE.WebGLRenderer({ antialias: false });
	frontendPerformanceDiagnostics.recordStageRendererCreated();
	frontendPerformanceDiagnostics.recordStageRendererCapabilities(
		rendererCapabilities(renderer),
	);
	renderer.setPixelRatio(Math.min(devicePixelRatio, pixelRatioCap ?? 1.25));
	renderer.outputColorSpace = THREE.SRGBColorSpace;
	renderer.shadowMap.enabled = true;
	renderer.shadowMap.type = THREE.PCFSoftShadowMap;
	renderer.shadowMap.autoUpdate = false;
	container.replaceChildren(renderer.domElement);
	const camera = new THREE.PerspectiveCamera(48, 1, 0.1, 100);
	if (retainedCameraPosition) camera.position.copy(retainedCameraPosition);
	const controls = new OrbitControls(camera, renderer.domElement);
	controls.enableDamping = true;
	controls.target.copy(retainedCameraTarget);
	cameraRef.current = camera;
	controlsRef.current = controls;
	const renderLoop = createStageRenderLoop({
		renderer,
		camera,
		controls,
		controller,
		diagnosticsRef,
		acknowledgeDesktopMirrorRender,
		onContextRecoveryFailed: () => remountAfterContextLoss(true),
		recordContextRecoveryOnNextRender: recoveringFromContextLoss,
	});
	const { requestRender, handleContextLost, handleContextRestored } =
		renderLoop;
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
	return (disposeRetainedScene: boolean) => {
		renderLoop.cancel();
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
		if (disposeRetainedScene) {
			const scene = controller.sceneRef.current;
			controller.sceneRef.current = null;
			controller.fixtureObjectsRef.current = new Map();
			if (scene) disposeScene(scene);
		}
		cameraRef.current = null;
		controlsRef.current = null;
		renderer.forceContextLoss();
		renderer.dispose();
		frontendPerformanceDiagnostics.recordStageRendererDisposed();
	};
}
