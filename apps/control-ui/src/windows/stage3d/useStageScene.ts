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
import { GLTFLoader } from "three/examples/jsm/loaders/GLTFLoader.js";
import type { StagePosition3d } from "../../api/ServerContext";
import type { VisualizationSnapshot } from "../../api/types";
import {
	buildStageScene,
	disposeScene,
	mountFixtureModel,
	type Stage3dFixture,
} from "../stage3dScene";

export type Stage3dCallbacks = {
	onSelect: (fixtureId: string, additive: boolean) => void;
	onMove: (fixtureId: string, position: StagePosition3d) => void;
	onMoveEnd: (fixtureId: string, position: StagePosition3d) => void;
};

export type StageSceneController = {
	sceneRef: MutableRefObject<THREE.Scene | null>;
	fixtureObjectsRef: MutableRefObject<Map<string, THREE.Object3D>>;
	positionByIdRef: MutableRefObject<Map<string, StagePosition3d>>;
	setupRef: MutableRefObject<boolean>;
	latestVisualizationRef: MutableRefObject<VisualizationSnapshot | null>;
	interactingRef: MutableRefObject<boolean>;
	callbacksRef: MutableRefObject<Stage3dCallbacks>;
	setRenderVisualization: Dispatch<
		SetStateAction<VisualizationSnapshot | null>
	>;
};

function retainFixtureModels(
	fixtures: Stage3dFixture[],
	previousObjects: Map<string, THREE.Object3D>,
	nextObjects: Map<string, THREE.Object3D>,
	selected: readonly string[],
	showSelection: boolean,
) {
	const retained = new Set<string>();
	for (const item of fixtures) {
		const instanceId = item.instanceId ?? item.fixture.fixture_id;
		const previousRoot = previousObjects.get(instanceId);
		const nextRoot = nextObjects.get(instanceId);
		const selectedNow =
			showSelection && selected.includes(item.fixture.fixture_id);
		if (
			!previousRoot ||
			!nextRoot ||
			Boolean(previousRoot.userData.stageSelected) !== selectedNow
		)
			continue;
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
	selected: readonly string[],
	showSelection: boolean,
) {
	let cancelled = false;
	for (const item of fixtures) {
		const instanceId = item.instanceId ?? item.fixture.fixture_id;
		if (retained.has(instanceId)) continue;
		const source = item.fixture.definition.model_asset;
		if (!source) continue;
		void fetch(source)
			.then((response) => response.arrayBuffer())
			.then((buffer) => {
				if (cancelled) return;
				new GLTFLoader().parse(buffer, "", (gltf) => {
					if (cancelled) return;
					const root = fixtureObjects.get(instanceId);
					if (!root) return;
					root.getObjectByName("fixture-placeholder")?.removeFromParent();
					mountFixtureModel(
						root,
						gltf.scene,
						item.fixture,
						showSelection && selected.includes(item.fixture.fixture_id),
					);
				});
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
	setup,
	showSelection,
	showFloorGrid,
	showBeamGuides,
	environmentBrightness,
	callbacks,
}: {
	fixtures: Stage3dFixture[];
	visualization: VisualizationSnapshot | null;
	selected: readonly string[];
	virtualHighlight: readonly string[];
	setup: boolean;
	showSelection: boolean;
	showFloorGrid: boolean;
	showBeamGuides: boolean;
	environmentBrightness: number;
	callbacks: Stage3dCallbacks;
}): StageSceneController {
	const sceneRef = useRef<THREE.Scene | null>(null);
	const fixtureObjectsRef = useRef(new Map<string, THREE.Object3D>());
	const positionByIdRef = useRef(new Map<string, StagePosition3d>());
	const setupRef = useRef(setup);
	const latestVisualizationRef = useRef(visualization);
	const interactingRef = useRef(false);
	const callbacksRef = useRef(callbacks);
	const [renderVisualization, setRenderVisualization] = useState(visualization);
	callbacksRef.current = callbacks;
	setupRef.current = setup;

	useEffect(() => {
		latestVisualizationRef.current = visualization;
		if (!interactingRef.current) setRenderVisualization(visualization);
	}, [visualization]);

	useEffect(() => {
		const next = buildStageScene(
			fixtures,
			renderVisualization,
			showSelection ? new Set(selected) : new Set(),
			environmentBrightness,
			showFloorGrid,
			showBeamGuides,
			new Set(virtualHighlight),
		);
		const previousScene = sceneRef.current;
		const retained = retainFixtureModels(
			fixtures,
			fixtureObjectsRef.current,
			next.fixtureObjects,
			selected,
			showSelection,
		);
		const cancelModels = loadFixtureModels(
			fixtures,
			next.fixtureObjects,
			retained,
			selected,
			showSelection,
		);
		positionByIdRef.current = new Map(
			fixtures.map((item) => [
				item.instanceId ?? item.fixture.fixture_id,
				item.position,
			]),
		);
		sceneRef.current = next.scene;
		fixtureObjectsRef.current = next.fixtureObjects;
		if (previousScene) disposeScene(previousScene);
		return cancelModels;
	}, [
		fixtures,
		renderVisualization,
		selected,
		virtualHighlight,
		showSelection,
		showFloorGrid,
		showBeamGuides,
		environmentBrightness,
	]);

	return useMemo(
		() => ({
			sceneRef,
			fixtureObjectsRef,
			positionByIdRef,
			setupRef,
			latestVisualizationRef,
			interactingRef,
			callbacksRef,
			setRenderVisualization,
		}),
		[],
	);
}
