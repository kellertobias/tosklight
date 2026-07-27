import * as THREE from "three";
import type { VisualizationSnapshot } from "../../api/types";
import type { StageRenderQuality } from "../../types";
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
				renderQuality: context.renderQuality,
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
			context.renderQuality,
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
	renderQuality: StageRenderQuality,
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
		renderQuality,
	};
}

function addGroundFootprints(
	scene: THREE.Scene,
	renderQuality: StageRenderQuality,
) {
	if (renderQuality !== "lines_only" && renderQuality !== "lines_and_beams")
		return;
	scene.updateMatrixWorld(true);
	const beams: THREE.Object3D[] = [];
	scene.traverse((object) => {
		if (
			object.userData.stageDirectionalBeam === true &&
			object.userData.stageBeamActive === true
		)
			beams.push(object);
	});
	for (const beam of beams) {
		const origin = beam.getWorldPosition(new THREE.Vector3());
		const direction = new THREE.Vector3(0, -1, 0)
			.applyQuaternion(beam.getWorldQuaternion(new THREE.Quaternion()))
			.normalize();
		if (direction.y >= -0.001) continue;
		const distanceToGround = -origin.y / direction.y;
		if (distanceToGround <= 0) continue;
		const referenceRadius = Number(beam.userData.stageBeamRadius);
		const referenceDistance = Number(beam.userData.stageBeamDistance);
		if (
			!Number.isFinite(referenceRadius) ||
			!Number.isFinite(referenceDistance) ||
			referenceDistance <= 0
		)
			continue;
		const radius = (referenceRadius / referenceDistance) * distanceToGround;
		const along = new THREE.Vector3(direction.x, 0, direction.z);
		if (along.lengthSq() < 1e-8) along.set(0, 0, 1);
		else along.normalize();
		const across = new THREE.Vector3(-along.z, 0, along.x);
		const center = origin.clone().addScaledVector(direction, distanceToGround);
		center.y = 0.006;
		const alongRadius = Math.min(
			radius / Math.max(0.08, Math.abs(direction.y)),
			radius * 12,
		);
		const points = Array.from({ length: 49 }, (_, index) => {
			const angle = (index / 48) * Math.PI * 2;
			return center
				.clone()
				.addScaledVector(along, Math.cos(angle) * alongRadius)
				.addScaledVector(across, Math.sin(angle) * radius);
		});
		const footprint = new THREE.LineLoop(
			new THREE.BufferGeometry().setFromPoints(points),
			new THREE.LineBasicMaterial({
				color: new THREE.Color(
					String(beam.userData.stageBeamColor ?? "#ffffff"),
				),
				transparent: true,
				opacity: 0.62,
			}),
		);
		footprint.name = "beam-ground-footprint";
		footprint.userData.stageBeamSource = beam.uuid;
		scene.add(footprint);
	}
}

export function buildStageScene(
	fixtures: Stage3dFixture[],
	snapshot: VisualizationSnapshot | null,
	selected: Set<string> = new Set(),
	environmentBrightness = 1,
	showFloorGrid = true,
	showBeamGuides = true,
	virtualHighlight: Set<string> = new Set(),
	renderQuality: StageRenderQuality = "lines_and_beams",
) {
	const scene = createStageEnvironment(environmentBrightness, showFloorGrid);
	const context = createSceneContext(
		snapshot,
		selected,
		showBeamGuides,
		virtualHighlight,
		renderQuality,
	);
	const fixtureObjects = new Map<string, THREE.Object3D>();
	for (const item of fixtures) {
		const { root, instanceId } = buildStageFixture(item, context);
		scene.add(root);
		fixtureObjects.set(instanceId, root);
	}
	addGroundFootprints(scene, renderQuality);
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
