import type { MutableRefObject } from "react";
import type * as THREE from "three";
import type { VisualizationSnapshot } from "../../api/types";
import { frontendPerformanceDiagnostics } from "../../features/frontendWarmup/diagnostics";
import type { StageRenderQuality } from "../../types";
import {
	buildStageScene,
	disposeScene,
	reconcileStageFixtures,
	type Stage3dFixture,
} from "../stage3dScene";
import type { StageProceduralResourceCache } from "../stage3dScene/resources";
import type { StageModelCache } from "./modelCache";
import {
	type MountedModelLease,
	refreshMountedModelLighting,
	updateMountedFixtureModels,
} from "./sceneModels";

export type StageSceneConfiguration = {
	environmentBrightness: number;
	showFloorGrid: boolean;
	contextRecoveryGeneration: number;
};

type StructureOptions = {
	fixtures: Stage3dFixture[];
	showFloorGrid: boolean;
	showBeamGuides: boolean;
	renderQuality: StageRenderQuality;
	environmentBrightness: number;
	contextRecoveryGeneration: number;
	selectedFixtures: Set<string>;
	highlightedFixtures: Set<string>;
	sceneRef: MutableRefObject<THREE.Scene | null>;
	fixtureObjectsRef: MutableRefObject<Map<string, THREE.Object3D>>;
	latestVisualizationRef: MutableRefObject<VisualizationSnapshot | null>;
	mountedModelsRef: MutableRefObject<Map<string, MountedModelLease>>;
	sceneConfigurationRef: MutableRefObject<StageSceneConfiguration | null>;
	appliedRenderQualityRef: MutableRefObject<StageRenderQuality>;
	invalidateRef: MutableRefObject<((immediate?: boolean) => void) | null>;
	modelCache: StageModelCache;
	resources: StageProceduralResourceCache;
	isSelected: (fixtureId: string) => boolean;
};

export function updateStageStructure(options: StructureOptions) {
	const startedAt = performance.now();
	const configuration = options.sceneConfigurationRef.current;
	const canReconcile =
		options.sceneRef.current !== null &&
		configuration?.environmentBrightness === options.environmentBrightness &&
		configuration.showFloorGrid === options.showFloorGrid &&
		configuration.contextRecoveryGeneration ===
			options.contextRecoveryGeneration;
	const result = canReconcile
		? reconcileExistingScene(options)
		: rebuildScene(options);
	options.sceneConfigurationRef.current = {
		environmentBrightness: options.environmentBrightness,
		showFloorGrid: options.showFloorGrid,
		contextRecoveryGeneration: options.contextRecoveryGeneration,
	};
	updateMountedFixtureModels({
		...result,
		fixtureObjects: options.fixtureObjectsRef.current,
		mountedModels: options.mountedModelsRef.current,
		isSelected: options.isSelected,
		modelCache: options.modelCache,
		onMounted: () =>
			refreshMountedModelLighting(
				options.sceneRef.current,
				options.appliedRenderQualityRef.current,
				options.invalidateRef.current,
			),
	});
	const structuralWork =
		!canReconcile ||
		result.changedFixtures.length > 0 ||
		result.removedInstanceIds.length > 0;
	if (structuralWork)
		recordSceneBuild(result.scene, options.fixtures.length, startedAt);
	options.invalidateRef.current?.();
}

function reconcileExistingScene(options: StructureOptions) {
	const scene = options.sceneRef.current as THREE.Scene;
	return {
		scene,
		...reconcileStageFixtures(
			scene,
			options.fixtureObjectsRef.current,
			options.fixtures,
			options.latestVisualizationRef.current,
			options.selectedFixtures,
			options.showBeamGuides,
			options.highlightedFixtures,
			options.renderQuality,
			options.resources,
		),
	};
}

function rebuildScene(options: StructureOptions) {
	const next = buildStageScene(
		options.fixtures,
		options.latestVisualizationRef.current,
		options.selectedFixtures,
		options.environmentBrightness,
		options.showFloorGrid,
		options.showBeamGuides,
		options.highlightedFixtures,
		options.renderQuality,
		options.resources,
	);
	const previousScene = options.sceneRef.current;
	options.sceneRef.current = next.scene;
	options.fixtureObjectsRef.current = next.fixtureObjects;
	options.appliedRenderQualityRef.current = options.renderQuality;
	if (previousScene) {
		disposeScene(previousScene);
		frontendPerformanceDiagnostics.recordStageSceneDisposal();
	}
	const nextIds = new Set(
		options.fixtures.map((item) => item.instanceId ?? item.fixture.fixture_id),
	);
	return {
		scene: next.scene,
		changedFixtures: options.fixtures,
		removedInstanceIds: [...options.mountedModelsRef.current.keys()].filter(
			(instanceId) => !nextIds.has(instanceId),
		),
	};
}

function recordSceneBuild(
	scene: THREE.Scene,
	fixtureCount: number,
	startedAt: number,
) {
	let objectCount = 0;
	const geometries = new Set<THREE.BufferGeometry>();
	const materials = new Set<THREE.Material>();
	scene.traverse((object) => {
		objectCount++;
		const mesh = object as THREE.Mesh;
		if (mesh.geometry) geometries.add(mesh.geometry);
		const objectMaterials = Array.isArray(mesh.material)
			? mesh.material
			: mesh.material
				? [mesh.material]
				: [];
		for (const material of objectMaterials) materials.add(material);
	});
	const finishedAt = performance.now();
	frontendPerformanceDiagnostics.recordStageSceneBuild({
		startedAt,
		finishedAt,
		durationMs: finishedAt - startedAt,
		fixtureCount,
		objectCount,
		geometryCount: geometries.size,
		materialCount: materials.size,
	});
}
