import type { MutableRefObject } from "react";
import * as THREE from "three";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { StageSceneController } from "./sceneTypes";

const lifecycleMocks = vi.hoisted(() => ({
	renderers: [] as Array<{
		domElement: HTMLCanvasElement;
		forceContextLoss: ReturnType<typeof vi.fn>;
		dispose: ReturnType<typeof vi.fn>;
	}>,
	controls: [] as Array<{
		target: THREE.Vector3;
		dispose: ReturnType<typeof vi.fn>;
	}>,
	renderLoopOptions: [] as Array<{
		onContextRecoveryFailed?: () => void;
		recordContextRecoveryOnNextRender?: boolean;
	}>,
	disposeScene: vi.fn(),
}));

vi.mock("three", async (importOriginal) => {
	const actual = await importOriginal<typeof import("three")>();
	class WebGLRenderer {
		domElement = document.createElement("canvas");
		shadowMap = { enabled: false, type: 0, autoUpdate: true };
		forceContextLoss = vi.fn();
		dispose = vi.fn();
		setPixelRatio = vi.fn();
		setSize = vi.fn();
		outputColorSpace = "";

		constructor() {
			lifecycleMocks.renderers.push(this);
		}
	}
	return { ...actual, WebGLRenderer };
});

vi.mock("three/examples/jsm/controls/OrbitControls.js", async () => {
	const { Vector3 } = await vi.importActual<typeof import("three")>("three");
	class OrbitControls {
		enableDamping = false;
		target = new Vector3();
		dispose = vi.fn();
		addEventListener = vi.fn();
		removeEventListener = vi.fn();

		constructor() {
			lifecycleMocks.controls.push(this);
		}
	}
	return { OrbitControls };
});

vi.mock("./rendererDiagnostics", () => ({
	rendererCapabilities: () => ({
		isWebGL2: false,
		precision: "highp",
		maxTextures: 16,
		maxTextureSize: 4_096,
		maxRenderbufferSize: 4_096,
		renderer: "test",
		vendor: "test",
	}),
}));

vi.mock("./rendererRenderLoop", () => ({
	createStageRenderLoop: vi.fn(
		(options: {
			onContextRecoveryFailed?: () => void;
			recordContextRecoveryOnNextRender?: boolean;
		}) => {
			lifecycleMocks.renderLoopOptions.push(options);
			return {
				requestRender: vi.fn(),
				cancel: vi.fn(),
				handleContextLost: vi.fn(),
				handleContextRestored: vi.fn(),
			};
		},
	),
}));

vi.mock("./pointerInteraction", () => ({
	bindStagePointerInteraction: () => vi.fn(),
}));

vi.mock("../stage3dScene", () => ({
	disposeScene: lifecycleMocks.disposeScene,
}));

import { mountStageRenderer } from "./rendererLifecycle";

beforeEach(() => {
	vi.clearAllMocks();
	lifecycleMocks.renderers.length = 0;
	lifecycleMocks.controls.length = 0;
	lifecycleMocks.renderLoopOptions.length = 0;
	vi.stubGlobal(
		"ResizeObserver",
		class {
			observe() {}
			disconnect() {}
		},
	);
});

afterEach(() => {
	vi.unstubAllGlobals();
});

describe("mountStageRenderer context recovery", () => {
	it("replaces the renderer while retaining scene and camera state", () => {
		const scene = new THREE.Scene();
		const cameraRef = {
			current: null,
		} as MutableRefObject<THREE.PerspectiveCamera | null>;
		const controlsRef = {
			current: null,
		} as MutableRefObject<{
			target: THREE.Vector3;
		} | null>;
		const controller = {
			sceneRef: { current: scene },
			fixtureObjectsRef: { current: new Map() },
			invalidateRef: { current: null },
		} as unknown as StageSceneController;
		const cleanup = mountStageRenderer({
			container: document.createElement("div"),
			controller,
			dispatch: vi.fn(),
			diagnosticsRef: {
				current: { lane: "normal", paneId: "stage" },
			},
			cameraRef,
			controlsRef: controlsRef as never,
			cameraTargetRef: { current: new THREE.Vector3(0, 1.8, -4) },
			acknowledgeDesktopMirrorRender: null,
		});
		cameraRef.current?.position.set(4, 5, 6);
		controlsRef.current?.target.set(1, 2, 3);

		lifecycleMocks.renderLoopOptions[0]?.onContextRecoveryFailed?.();

		expect(lifecycleMocks.renderers).toHaveLength(2);
		expect(lifecycleMocks.renderers[0]?.dispose).toHaveBeenCalledOnce();
		expect(lifecycleMocks.controls[0]?.dispose).toHaveBeenCalledOnce();
		expect(controller.sceneRef.current).toBe(scene);
		expect(lifecycleMocks.disposeScene).not.toHaveBeenCalled();
		expect(cameraRef.current?.position.toArray()).toEqual([4, 5, 6]);
		expect(controlsRef.current?.target.toArray()).toEqual([1, 2, 3]);
		expect(
			lifecycleMocks.renderLoopOptions[1]
				?.recordContextRecoveryOnNextRender,
		).toBe(true);

		cleanup();

		expect(lifecycleMocks.disposeScene).toHaveBeenCalledOnce();
		expect(lifecycleMocks.disposeScene).toHaveBeenCalledWith(scene);
		expect(controller.sceneRef.current).toBeNull();
		expect(lifecycleMocks.renderers[1]?.dispose).toHaveBeenCalledOnce();
	});
});
