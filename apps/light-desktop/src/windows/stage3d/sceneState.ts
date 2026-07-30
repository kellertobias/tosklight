import { useEffect, useRef } from "react";
import type * as THREE from "three";
import type { VisualizationSnapshot } from "../../api/types";
import type { StageRenderQuality } from "../../types";
import { StageProceduralResourceCache } from "../stage3dScene/resources";
import { StageModelCache } from "./modelCache";
import type { MountedModelLease } from "./sceneModels";
import type { StageSceneConfiguration } from "./sceneStructureLifecycle";
import type { Stage3dCallbacks } from "./sceneTypes";

export function useStageSceneState(
	visualization: VisualizationSnapshot | null,
	renderQuality: StageRenderQuality,
	callbacks: Stage3dCallbacks,
) {
	const state = {
		sceneRef: useRef<THREE.Scene | null>(null),
		fixtureObjectsRef: useRef(new Map<string, THREE.Object3D>()),
		latestVisualizationRef: useRef(visualization),
		interactingRef: useRef(false),
		callbacksRef: useRef(callbacks),
		invalidateRef: useRef<((immediate?: boolean) => void) | null>(null),
		modelCacheRef: useRef<StageModelCache | null>(null),
		resourceCacheRef: useRef<StageProceduralResourceCache | null>(null),
		mountedModelsRef: useRef(new Map<string, MountedModelLease>()),
		sceneConfigurationRef: useRef<StageSceneConfiguration | null>(null),
		displayedVisualizationRef: useRef(visualization),
		visualizationSettledRef: useRef(true),
		appliedRenderQualityRef: useRef(renderQuality),
		installVisualizationRef: useRef<
			(
				snapshot: VisualizationSnapshot | null,
				forceVisibleApply?: boolean,
			) => void
		>(() => undefined),
	};
	if (!state.modelCacheRef.current)
		state.modelCacheRef.current = new StageModelCache();
	if (!state.resourceCacheRef.current)
		state.resourceCacheRef.current = new StageProceduralResourceCache();
	state.callbacksRef.current = callbacks;
	const modelCache = state.modelCacheRef.current;
	const resources = state.resourceCacheRef.current;
	useEffect(
		() => () => {
			for (const mounted of state.mountedModelsRef.current.values())
				mounted.release();
			state.mountedModelsRef.current.clear();
			state.modelCacheRef.current?.dispose();
			state.modelCacheRef.current = null;
			state.resourceCacheRef.current?.dispose();
			state.resourceCacheRef.current = null;
		},
		[],
	);
	return { ...state, modelCache, resources };
}
