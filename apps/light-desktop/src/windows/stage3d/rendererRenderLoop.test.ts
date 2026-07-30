import type { MutableRefObject } from "react";
import * as THREE from "three";
import { afterEach, describe, expect, it, vi } from "vitest";
import { frontendPerformanceDiagnostics } from "../../features/frontendWarmup/diagnostics";
import { createStageRenderLoop } from "./rendererRenderLoop";
import type { StageSceneController } from "./sceneTypes";

afterEach(() => {
	vi.useRealTimers();
	vi.unstubAllGlobals();
});

describe("createStageRenderLoop context recovery", () => {
	it("requests native restoration and records recovery after one submitted frame", () => {
		vi.useFakeTimers();
		let scheduledFrame: FrameRequestCallback | null = null;
		vi.stubGlobal(
			"requestAnimationFrame",
			vi.fn((callback: FrameRequestCallback) => {
				scheduledFrame = callback;
				return 1;
			}),
		);
		vi.stubGlobal("cancelAnimationFrame", vi.fn());
		const baseline =
			frontendPerformanceDiagnostics.snapshot().stage.rendererContextRestores;
		const scene = new THREE.Scene();
		let contextLost = true;
		const restoreContext = vi.fn(() => {
			contextLost = false;
		});
		const renderer = {
			shadowMap: { needsUpdate: false },
			render: vi.fn(),
			resetState: vi.fn(),
			getContext: () => ({
				getExtension: () => ({ restoreContext }),
				isContextLost: () => contextLost,
			}),
			info: {
				render: { calls: 0, triangles: 0, lines: 0, points: 0 },
				memory: { geometries: 0, textures: 0 },
			},
		};
		const controller = {
			sceneRef: { current: scene },
			appliedRenderQualityRef: { current: "lines_and_beams" },
			displayedVisualizationRef: { current: null },
			visualizationSettledRef: { current: true },
			recoverContext: vi.fn(),
		} as unknown as StageSceneController;
		const renderLoop = createStageRenderLoop({
			renderer: renderer as unknown as THREE.WebGLRenderer,
			camera: new THREE.PerspectiveCamera(),
			controls: {
				update: () => false,
			} as never,
			controller,
			diagnosticsRef: {
				current: { lane: "normal", paneId: "stage" },
			} as MutableRefObject<{
				lane: "normal" | "preload";
				paneId: string | null;
			}>,
			acknowledgeDesktopMirrorRender: null,
		});
		const preventDefault = vi.fn();

		renderLoop.handleContextLost({ preventDefault } as unknown as Event);

		expect(preventDefault).toHaveBeenCalledOnce();

		renderLoop.requestRender();
		expect(scheduledFrame).toBeNull();
		vi.advanceTimersByTime(50);
		expect(restoreContext).toHaveBeenCalledOnce();
		vi.advanceTimersByTime(50);

		expect(renderer.render).toHaveBeenCalledOnce();
		expect(
			frontendPerformanceDiagnostics.snapshot().stage.rendererContextRestores -
				baseline,
		).toBe(1);

		scheduledFrame = null;
		renderLoop.requestRender();
		(scheduledFrame as unknown as FrameRequestCallback)(1);
		expect(
			frontendPerformanceDiagnostics.snapshot().stage.rendererContextRestores -
				baseline,
		).toBe(1);
		expect(controller.recoverContext).toHaveBeenCalledOnce();
	});

	it("requests renderer replacement after native restoration remains lost", () => {
		vi.useFakeTimers();
		vi.stubGlobal("requestAnimationFrame", vi.fn(() => 1));
		vi.stubGlobal("cancelAnimationFrame", vi.fn());
		const restoreContext = vi.fn();
		const renderer = {
			shadowMap: { needsUpdate: false },
			resetState: vi.fn(),
			getContext: () => ({
				getExtension: () => ({ restoreContext }),
				isContextLost: () => true,
			}),
		};
		const onContextRecoveryFailed = vi.fn();
		const renderLoop = createStageRenderLoop({
			renderer: renderer as unknown as THREE.WebGLRenderer,
			camera: new THREE.PerspectiveCamera(),
			controls: { update: () => false } as never,
			controller: {
				sceneRef: { current: new THREE.Scene() },
			} as unknown as StageSceneController,
			diagnosticsRef: {
				current: { lane: "normal", paneId: "stage" },
			} as MutableRefObject<{
				lane: "normal" | "preload";
				paneId: string | null;
			}>,
			acknowledgeDesktopMirrorRender: null,
			onContextRecoveryFailed,
		});

		renderLoop.handleContextLost({
			preventDefault: vi.fn(),
		} as unknown as Event);
		vi.advanceTimersByTime(1_499);

		expect(restoreContext).toHaveBeenCalledTimes(3);
		expect(onContextRecoveryFailed).not.toHaveBeenCalled();

		vi.advanceTimersByTime(1);
		expect(onContextRecoveryFailed).toHaveBeenCalledOnce();

		vi.advanceTimersByTime(5_000);
		expect(onContextRecoveryFailed).toHaveBeenCalledOnce();
	});

	it("records replacement recovery only after the retained scene renders", () => {
		let scheduledFrame: FrameRequestCallback | null = null;
		vi.stubGlobal(
			"requestAnimationFrame",
			vi.fn((callback: FrameRequestCallback) => {
				scheduledFrame = callback;
				return 1;
			}),
		);
		vi.stubGlobal("cancelAnimationFrame", vi.fn());
		const baseline =
			frontendPerformanceDiagnostics.snapshot().stage.rendererContextRestores;
		const renderer = {
			shadowMap: { needsUpdate: false },
			render: vi.fn(),
			info: {
				render: { calls: 0, triangles: 0, lines: 0, points: 0 },
				memory: { geometries: 0, textures: 0 },
			},
		};
		const scene = new THREE.Scene();
		const renderLoop = createStageRenderLoop({
			renderer: renderer as unknown as THREE.WebGLRenderer,
			camera: new THREE.PerspectiveCamera(),
			controls: { update: () => false } as never,
			controller: {
				sceneRef: { current: scene },
				appliedRenderQualityRef: { current: "lines_and_beams" },
				displayedVisualizationRef: { current: null },
				visualizationSettledRef: { current: true },
			} as unknown as StageSceneController,
			diagnosticsRef: {
				current: { lane: "normal", paneId: "stage" },
			} as MutableRefObject<{
				lane: "normal" | "preload";
				paneId: string | null;
			}>,
			acknowledgeDesktopMirrorRender: null,
			recordContextRecoveryOnNextRender: true,
		});

		expect(
			frontendPerformanceDiagnostics.snapshot().stage.rendererContextRestores -
				baseline,
		).toBe(0);

		renderLoop.requestRender();
		(scheduledFrame as unknown as FrameRequestCallback)(1);

		expect(renderer.render).toHaveBeenCalledWith(scene, expect.anything());
		expect(
			frontendPerformanceDiagnostics.snapshot().stage.rendererContextRestores -
				baseline,
		).toBe(1);

		scheduledFrame = null;
		renderLoop.requestRender();
		(scheduledFrame as unknown as FrameRequestCallback)(2);
		expect(
			frontendPerformanceDiagnostics.snapshot().stage.rendererContextRestores -
				baseline,
		).toBe(1);
	});
});
