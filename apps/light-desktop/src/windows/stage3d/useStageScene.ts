import { useEffect, useMemo, useRef, useState } from "react";
import type { VisualizationSnapshot } from "../../api/types";
import type { StageRenderQuality } from "../../types";
import type { Stage3dFixture } from "../stage3dScene";
import { useStageSceneState } from "./sceneState";
import { updateStageStructure } from "./sceneStructureLifecycle";
import type { Stage3dCallbacks, StageSceneController } from "./sceneTypes";
import { useStageVisualizationLifecycle } from "./sceneVisualizationLifecycle";

export { retainFixtureModel } from "./sceneModels";
export type { Stage3dCallbacks, StageSceneController } from "./sceneTypes";

export function useStageScene({
	fixtures,
	visualization,
	selected,
	virtualHighlight,
	showSelection,
	showFloorGrid,
	showBeamGuides,
	renderQuality,
	environmentBrightness,
	callbacks,
}: {
	fixtures: Stage3dFixture[];
	visualization: VisualizationSnapshot | null;
	selected: readonly string[];
	virtualHighlight: readonly string[];
	showSelection: boolean;
	showFloorGrid: boolean;
	showBeamGuides: boolean;
	renderQuality: StageRenderQuality;
	environmentBrightness: number;
	callbacks: Stage3dCallbacks;
}): StageSceneController {
	const state = useStageSceneState(visualization, renderQuality, callbacks);
	const {
		sceneRef,
		fixtureObjectsRef,
		latestVisualizationRef,
		interactingRef,
		callbacksRef,
		invalidateRef,
		modelCache,
		resources,
		mountedModelsRef,
		sceneConfigurationRef,
		displayedVisualizationRef,
		visualizationSettledRef,
		appliedRenderQualityRef,
		installVisualizationRef,
	} = state;
	const selectionRef = useRef({ selected, showSelection });
	const [contextRecoveryGeneration, setContextRecoveryGeneration] = useState(0);
	selectionRef.current = { selected, showSelection };

	useEffect(() => {
		latestVisualizationRef.current = visualization;
		if (!interactingRef.current) installVisualizationRef.current(visualization);
	}, [visualization]);

	useEffect(() => {
		updateStageStructure({
			fixtures,
			showFloorGrid,
			showBeamGuides,
			renderQuality,
			environmentBrightness,
			contextRecoveryGeneration,
			selectedFixtures: showSelection ? new Set(selected) : new Set(),
			highlightedFixtures: new Set(virtualHighlight),
			sceneRef,
			fixtureObjectsRef,
			latestVisualizationRef,
			mountedModelsRef,
			sceneConfigurationRef,
			appliedRenderQualityRef,
			invalidateRef,
			modelCache,
			resources,
			isSelected: (fixtureId) =>
				selectionRef.current.showSelection &&
				selectionRef.current.selected.includes(fixtureId),
		});
	}, [
		fixtures,
		showFloorGrid,
		environmentBrightness,
		contextRecoveryGeneration,
	]);

	useStageVisualizationLifecycle({
		fixtures,
		showBeamGuides,
		renderQuality,
		selected,
		selectedKey: selected.join("\u0000"),
		showSelection,
		virtualHighlight,
		virtualHighlightKey: virtualHighlight.join("\u0000"),
		fixtureObjectsRef,
		latestVisualizationRef,
		displayedVisualizationRef,
		visualizationSettledRef,
		appliedRenderQualityRef,
		interactingRef,
		sceneRef,
		invalidateRef,
		installVisualizationRef,
	});

	return useMemo(
		() => ({
			sceneRef,
			fixtureObjectsRef,
			latestVisualizationRef,
			interactingRef,
			callbacksRef,
			invalidateRef,
			displayedVisualizationRef,
			visualizationSettledRef,
			appliedRenderQualityRef,
			installVisualization: (snapshot) =>
				installVisualizationRef.current(snapshot),
			recoverContext: () =>
				setContextRecoveryGeneration((generation) => generation + 1),
		}),
		[],
	);
}
