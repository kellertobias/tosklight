import {
	type Dispatch,
	type MutableRefObject,
	type SetStateAction,
	useEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import type * as THREE from "three";
import type { VisualizationSnapshot } from "../../api/types";
import { frontendPerformanceDiagnostics } from "../../features/frontendWarmup/diagnostics";
import type { StageRenderQuality } from "../../types";
import {
	applyStageVisualization,
	buildStageScene,
	disposeScene,
	mountFixtureModel,
	type Stage3dFixture,
} from "../stage3dScene";
import {
	interpolateVisualizationSnapshot,
	STAGE_INTERPOLATION_MILLIS,
} from "./interpolation";
import { StageModelCache } from "./modelCache";

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
	setRenderVisualization: Dispatch<
		SetStateAction<VisualizationSnapshot | null>
	>;
};

function retainFixtureModels(
	fixtures: Stage3dFixture[],
	previousObjects: Map<string, THREE.Object3D>,
	nextObjects: Map<string, THREE.Object3D>,
) {
	const retained = new Set<string>();
	for (const item of fixtures) {
		const instanceId = item.instanceId ?? item.fixture.fixture_id;
		const previousRoot = previousObjects.get(instanceId);
		const nextRoot = nextObjects.get(instanceId);
		if (!previousRoot || !nextRoot) continue;
		const mounted = previousRoot.children.filter(
			(child) =>
				child.name === "fixture-model" ||
				child.name.startsWith("fixture-model-part:"),
		);
		previousRoot.traverse((child) => {
			if (
				child.parent !== previousRoot &&
				(child.name === "fixture-model" ||
					child.name.startsWith("fixture-model-part:"))
			)
				mounted.push(child);
		});
		for (const model of [...new Set(mounted)]) {
			const target = model.parent?.name
				? nextRoot.getObjectByName(model.parent.name)
				: nextRoot;
			if (!target) continue;
			if (model.name === "fixture-model")
				target.getObjectByName("fixture-placeholder")?.removeFromParent();
			else
				target
					.getObjectByName(
						`geometry-part:${model.name.slice("fixture-model-part:".length)}`,
					)
					?.removeFromParent();
			target.add(model);
			retained.add(instanceId);
		}
	}
	return retained;
}

function loadFixtureModels(
	fixtures: Stage3dFixture[],
	fixtureObjects: Map<string, THREE.Object3D>,
	retained: Set<string>,
	isSelected: (fixtureId: string) => boolean,
	modelCache: StageModelCache,
) {
	let cancelled = false;
	for (const item of fixtures) {
		const instanceId = item.instanceId ?? item.fixture.fixture_id;
		if (retained.has(instanceId)) continue;
		const source = item.fixture.definition.model_asset;
		if (!source) continue;
		void modelCache
			.clone(source)
			.then((model) => {
				if (cancelled) return;
				const root = fixtureObjects.get(instanceId);
				if (!root) return;
				root.getObjectByName("fixture-placeholder")?.removeFromParent();
				mountFixtureModel(
					root,
					model,
					item.fixture,
					isSelected(item.fixture.fixture_id),
				);
			})
			.catch(() => undefined);
	}
	return () => {
		cancelled = true;
	};
}

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
	const sceneRef = useRef<THREE.Scene | null>(null);
	const fixtureObjectsRef = useRef(new Map<string, THREE.Object3D>());
	const latestVisualizationRef = useRef(visualization);
	const interactingRef = useRef(false);
	const callbacksRef = useRef(callbacks);
	const invalidateRef = useRef<(() => void) | null>(null);
	const modelCacheRef = useRef<StageModelCache | null>(null);
	const selectionRef = useRef({ selected, showSelection });
	const displayedVisualizationRef = useRef(visualization);
	if (!modelCacheRef.current) modelCacheRef.current = new StageModelCache();
	const modelCache = modelCacheRef.current;
	const [renderVisualization, setRenderVisualization] = useState(visualization);
	callbacksRef.current = callbacks;
	selectionRef.current = { selected, showSelection };

	useEffect(() => {
		latestVisualizationRef.current = visualization;
		if (!interactingRef.current) setRenderVisualization(visualization);
	}, [visualization]);

	useEffect(() => {
		const startedAt = performance.now();
		const next = buildStageScene(
			fixtures,
			renderVisualization,
			showSelection ? new Set(selected) : new Set(),
			environmentBrightness,
			showFloorGrid,
			showBeamGuides,
			new Set(virtualHighlight),
			renderQuality,
		);
		let objectCount = 0;
		let geometryCount = 0;
		let materialCount = 0;
		const geometries = new Set<THREE.BufferGeometry>();
		const materials = new Set<THREE.Material>();
		next.scene.traverse((object) => {
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
		geometryCount = geometries.size;
		materialCount = materials.size;
		const finishedAt = performance.now();
		frontendPerformanceDiagnostics.recordStageSceneBuild({
			startedAt,
			finishedAt,
			durationMs: finishedAt - startedAt,
			fixtureCount: fixtures.length,
			objectCount,
			geometryCount,
			materialCount,
		});
		const previousScene = sceneRef.current;
		const retained = retainFixtureModels(
			fixtures,
			fixtureObjectsRef.current,
			next.fixtureObjects,
		);
		const cancelModels = loadFixtureModels(
			fixtures,
			next.fixtureObjects,
			retained,
			(fixtureId) =>
				selectionRef.current.showSelection &&
				selectionRef.current.selected.includes(fixtureId),
			modelCache,
		);
		sceneRef.current = next.scene;
		fixtureObjectsRef.current = next.fixtureObjects;
		invalidateRef.current?.();
		if (previousScene) {
			disposeScene(previousScene);
			frontendPerformanceDiagnostics.recordStageSceneDisposal();
		}
		return cancelModels;
	}, [fixtures, showFloorGrid, environmentBrightness, modelCache]);

	useEffect(
		() => () => {
			modelCacheRef.current?.dispose();
			modelCacheRef.current = null;
		},
		[],
	);

	useEffect(() => {
		if (!sceneRef.current) return;
		const target = renderVisualization;
		const from = displayedVisualizationRef.current;
		let frame: number | null = null;
		const apply = (snapshot: VisualizationSnapshot | null) => {
			displayedVisualizationRef.current = snapshot;
			applyStageVisualization(
				fixtures,
				snapshot,
				fixtureObjectsRef.current,
				showBeamGuides,
				renderQuality,
				new Set(virtualHighlight),
				new Set(selected),
				showSelection,
			);
			invalidateRef.current?.();
		};
		if (!from || !target) {
			apply(target);
			return;
		}
		const startedAt = performance.now();
		const step = (now: number) => {
			const progress = Math.min(
				1,
				Math.max(0, (now - startedAt) / STAGE_INTERPOLATION_MILLIS),
			);
			apply(interpolateVisualizationSnapshot(from, target, progress));
			if (progress < 1) frame = requestAnimationFrame(step);
		};
		frame = requestAnimationFrame(step);
		return () => {
			if (frame !== null) cancelAnimationFrame(frame);
		};
	}, [
		fixtures,
		renderVisualization,
		showBeamGuides,
		renderQuality,
		selected,
		showSelection,
		virtualHighlight,
	]);

	return useMemo(
		() => ({
			sceneRef,
			fixtureObjectsRef,
			latestVisualizationRef,
			interactingRef,
			callbacksRef,
			invalidateRef,
			setRenderVisualization,
		}),
		[],
	);
}
