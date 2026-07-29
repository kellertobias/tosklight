import type { MutableRefObject } from "react";
import type * as THREE from "three";
import type { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { frontendPerformanceDiagnostics } from "../../features/frontendWarmup/diagnostics";
import {
	transparentStageDrawCalls,
	visibleStageObjects,
} from "./rendererDiagnostics";
import type { StageSceneController } from "./sceneTypes";

export function createStageRenderLoop({
	renderer,
	camera,
	controls,
	controller,
	diagnosticsRef,
	acknowledgeDesktopMirrorRender,
}: {
	renderer: THREE.WebGLRenderer;
	camera: THREE.PerspectiveCamera;
	controls: OrbitControls;
	controller: StageSceneController;
	diagnosticsRef: MutableRefObject<{
		lane: "normal" | "preload";
		paneId: string | null;
	}>;
	acknowledgeDesktopMirrorRender: (() => void) | null | undefined;
}) {
	let frame: number | null = null;
	let contextLost = false;
	const render = () => {
		frame = null;
		if (contextLost) return;
		frontendPerformanceDiagnostics.recordStageRafCallback();
		const controlsChanged = controls.update();
		const scene = controller.sceneRef.current;
		if (scene) {
			renderer.shadowMap.needsUpdate =
				scene.userData.stageImprovedShadowsDirty === true;
			const startedAt = performance.now();
			renderer.render(scene, camera);
			const submittedAt = performance.now();
			acknowledgeDesktopMirrorRender?.();
			scene.userData.stageImprovedShadowsDirty = false;
			renderer.shadowMap.needsUpdate = false;
			frontendPerformanceDiagnostics.recordStageRender({
				...diagnosticsRef.current,
				renderQuality: controller.appliedRenderQualityRef.current,
				visibleObjects: visibleStageObjects(scene),
				submittedAt,
				durationMs: submittedAt - startedAt,
				calls: renderer.info.render.calls,
				transparentDrawCalls: transparentStageDrawCalls(scene),
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
	return {
		requestRender,
		cancel: () => {
			if (frame !== null) cancelAnimationFrame(frame);
			frame = null;
		},
		handleContextLost: (event: Event) => {
			event.preventDefault();
			frontendPerformanceDiagnostics.recordStageRendererContextLost();
			contextLost = true;
			if (frame !== null) cancelAnimationFrame(frame);
			frame = null;
		},
		handleContextRestored: () => {
			contextLost = false;
			frontendPerformanceDiagnostics.recordStageRendererContextRestored();
			renderer.resetState();
			controller.recoverContext();
			if (controller.sceneRef.current)
				controller.sceneRef.current.userData.stageImprovedShadowsDirty = true;
			requestRender();
		},
	};
}
