import * as THREE from "three";
import type { VisualizationSnapshot } from "../../api/types";
import { profileMode, valuesByFixture } from "./attributeValues";
import {
	createFixtureRoot,
	fallbackRenderState,
	mountFallbackFixture,
} from "./fallbackFixture";
import { buildFixtureProfileGeometry } from "./profileGeometry";
import type { Stage3dFixture, StageSceneContext } from "./types";

function buildStageFixture(item: Stage3dFixture, context: StageSceneContext) {
	const fixtureId = item.fixture.fixture_id;
	const selected = context.selected.has(fixtureId);
	const attributes = context.byFixture.get(fixtureId) ?? new Map();
	const { root, instanceId } = createFixtureRoot(item, selected);
	const mode = profileMode(item.fixture);
	const profileGeometry = mode
		? buildFixtureProfileGeometry({
				fixture: item.fixture,
				mode,
				byFixture: context.byFixture,
				selected,
				snapshot: context.snapshot,
				projectedOwners: context.projectedOwners,
				showBeamGuides: context.showBeamGuides,
				virtualHighlight: context.virtualHighlight.has(fixtureId),
			})
		: null;
	if (profileGeometry) {
		root.add(profileGeometry);
	} else {
		mountFallbackFixture(
			root,
			item,
			attributes,
			fallbackRenderState(
				item,
				attributes,
				context.snapshot,
				context.virtualHighlight.has(fixtureId),
			),
			selected,
			context.showBeamGuides,
		);
	}
	return { root, instanceId };
}

function addStageFloor(scene: THREE.Scene) {
	const floor = new THREE.Mesh(
		new THREE.PlaneGeometry(12, 8),
		new THREE.MeshStandardMaterial({ color: 0x151b20, roughness: 0.9 }),
	);
	floor.name = "stage-floor";
	floor.rotation.x = -Math.PI / 2;
	floor.position.set(0, 0, -4);
	scene.add(floor);
	const grid = new THREE.GridHelper(12, 24, 0x24798a, 0x263039);
	grid.name = "stage-floor-grid";
	grid.position.z = -4;
	scene.add(grid);
}

function createStageEnvironment(
	environmentBrightness: number,
	showFloorGrid: boolean,
) {
	const scene = new THREE.Scene();
	scene.background = new THREE.Color(0x080b0f).lerp(
		new THREE.Color(0x26323a),
		environmentBrightness * 0.18,
	);
	scene.add(
		new THREE.HemisphereLight(0xa9c8dc, 0x11151a, environmentBrightness * 1.5),
	);
	if (showFloorGrid) addStageFloor(scene);
	return scene;
}

function createSceneContext(
	snapshot: VisualizationSnapshot | null,
	selected: Set<string>,
	showBeamGuides: boolean,
	virtualHighlight: Set<string>,
): StageSceneContext {
	return {
		snapshot,
		selected,
		byFixture: valuesByFixture(snapshot),
		projectedOwners: new Set(
			(snapshot?.profile_output_values ?? []).map((entry) => entry.fixture_id),
		),
		showBeamGuides,
		virtualHighlight,
	};
}

export function buildStageScene(
	fixtures: Stage3dFixture[],
	snapshot: VisualizationSnapshot | null,
	selected: Set<string> = new Set(),
	environmentBrightness = 1,
	showFloorGrid = true,
	showBeamGuides = true,
	virtualHighlight: Set<string> = new Set(),
) {
	const scene = createStageEnvironment(environmentBrightness, showFloorGrid);
	const context = createSceneContext(
		snapshot,
		selected,
		showBeamGuides,
		virtualHighlight,
	);
	const fixtureObjects = new Map<string, THREE.Object3D>();
	for (const item of fixtures) {
		const { root, instanceId } = buildStageFixture(item, context);
		scene.add(root);
		fixtureObjects.set(instanceId, root);
	}
	return { scene, fixtureObjects };
}

export function disposeScene(scene: THREE.Scene) {
	scene.traverse((object) => {
		const mesh = object as THREE.Mesh;
		mesh.geometry?.dispose();
		const materials = Array.isArray(mesh.material)
			? mesh.material
			: mesh.material
				? [mesh.material]
				: [];
		for (const material of materials) material.dispose();
	});
}
