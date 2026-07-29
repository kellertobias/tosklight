import type { MutableRefObject } from "react";
import type * as THREE from "three";
import type { VisualizationSnapshot } from "../../api/types";
import type { StageRenderQuality } from "../../types";

export type Stage3dCallbacks = {
	onSelect: (fixtureId: string, additive: boolean) => void;
};

export type StageSceneController = {
	sceneRef: MutableRefObject<THREE.Scene | null>;
	fixtureObjectsRef: MutableRefObject<Map<string, THREE.Object3D>>;
	latestVisualizationRef: MutableRefObject<VisualizationSnapshot | null>;
	interactingRef: MutableRefObject<boolean>;
	callbacksRef: MutableRefObject<Stage3dCallbacks>;
	invalidateRef: MutableRefObject<(() => void) | null>;
	displayedVisualizationRef: MutableRefObject<VisualizationSnapshot | null>;
	visualizationSettledRef: MutableRefObject<boolean>;
	appliedRenderQualityRef: MutableRefObject<StageRenderQuality>;
	installVisualization: (snapshot: VisualizationSnapshot | null) => void;
	recoverContext: () => void;
};
