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
	onContextRecoveryFailed,
	recordContextRecoveryOnNextRender = false,
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
	onContextRecoveryFailed?: () => void;
	recordContextRecoveryOnNextRender?: boolean;
}) {
	let frame: number | null = null;
	let contextLost = false;
	let contextRecoveryPending = recordContextRecoveryOnNextRender;
	let contextRecoveryFallbackRequested = false;
	const contextRestoreTimers = new Set<ReturnType<typeof setTimeout>>();
	const clearContextRestoreTimers = () => {
		for (const timer of contextRestoreTimers) clearTimeout(timer);
		contextRestoreTimers.clear();
	};
	const requestNativeContextRestore = () => {
		const context = renderer.getContext();
		const extension = context.getExtension(
			"WEBGL_lose_context",
		) as WEBGL_lose_context | null;
		extension?.restoreContext();
		const timer = setTimeout(() => {
			contextRestoreTimers.delete(timer);
			if (contextLost && !context.isContextLost()) completeContextRestore();
		}, 50);
		contextRestoreTimers.add(timer);
	};
	const completeContextRestore = () => {
		if (!contextLost) return;
		clearContextRestoreTimers();
		contextLost = false;
		contextRecoveryPending = true;
		renderer.resetState();
		controller.recoverContext();
		if (controller.sceneRef.current)
			controller.sceneRef.current.userData.stageImprovedShadowsDirty = true;
		requestRender(true);
	};
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
			if (contextRecoveryPending) {
				contextRecoveryPending = false;
				frontendPerformanceDiagnostics.recordStageRendererContextRestored(
					diagnosticsRef.current.lane,
				);
			}
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
	const requestRender = (immediate = false) => {
		if (contextLost) return;
		if (immediate) {
			if (frame !== null) cancelAnimationFrame(frame);
			frame = null;
			render();
		} else if (frame === null) {
			frame = requestAnimationFrame(render);
		}
	};
	return {
		requestRender,
		cancel: () => {
			if (frame !== null) cancelAnimationFrame(frame);
			frame = null;
			clearContextRestoreTimers();
		},
		handleContextLost: (event: Event) => {
			event.preventDefault();
			frontendPerformanceDiagnostics.recordStageRendererContextLost(
				diagnosticsRef.current.lane,
			);
			frontendPerformanceDiagnostics.invalidateUnsettledStageFrame(
				diagnosticsRef.current.lane,
			);
			contextLost = true;
			contextRecoveryFallbackRequested = false;
			if (frame !== null) cancelAnimationFrame(frame);
			frame = null;
			clearContextRestoreTimers();
			for (const delay of [50, 250, 750]) {
				const timer = setTimeout(() => {
					contextRestoreTimers.delete(timer);
					if (contextLost) requestNativeContextRestore();
				}, delay);
				contextRestoreTimers.add(timer);
			}
			const fallbackTimer = setTimeout(() => {
				contextRestoreTimers.delete(fallbackTimer);
				if (
					contextLost &&
					!contextRecoveryFallbackRequested &&
					onContextRecoveryFailed
				) {
					contextRecoveryFallbackRequested = true;
					clearContextRestoreTimers();
					onContextRecoveryFailed();
				}
			}, 1_500);
			contextRestoreTimers.add(fallbackTimer);
		},
		handleContextRestored: completeContextRestore,
	};
}
