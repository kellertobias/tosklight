import * as THREE from "three";
import type { VisualizationSnapshot } from "../../api/types";
import type { StageRenderQuality } from "../../types";
import { profileMode, valuesByFixture } from "./attributeValues";
import {
	createFixtureRoot,
	fallbackRenderState,
	mountFallbackFixture,
	updateFallbackFixture,
} from "./fallbackFixture";
import {
	buildFixtureProfileGeometry,
	updateFixtureProfileGeometry,
} from "./profileGeometry";
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

function updateGroundFootprint(
	footprint: THREE.LineLoop,
	beam: THREE.Object3D,
) {
	const active = beam.userData.stageBeamActive === true;
	footprint.visible = active;
	if (!active) return;
	const origin = beam.getWorldPosition(new THREE.Vector3());
	const direction = new THREE.Vector3(0, -1, 0)
		.applyQuaternion(beam.getWorldQuaternion(new THREE.Quaternion()))
		.normalize();
	if (direction.y >= -0.001) {
		footprint.visible = false;
		return;
	}
	const distanceToGround = -origin.y / direction.y;
	if (distanceToGround <= 0) {
		footprint.visible = false;
		return;
	}
	const referenceRadius = Number(beam.userData.stageBeamRadius);
	const referenceDistance = Number(beam.userData.stageBeamDistance);
	if (
		!Number.isFinite(referenceRadius) ||
		!Number.isFinite(referenceDistance) ||
		referenceDistance <= 0
	) {
		footprint.visible = false;
		return;
	}
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
	const position = footprint.geometry.getAttribute(
		"position",
	) as THREE.BufferAttribute;
	for (let index = 0; index < 49; index++) {
		const angle = (index / 48) * Math.PI * 2;
		const point = center
			.clone()
			.addScaledVector(along, Math.cos(angle) * alongRadius)
			.addScaledVector(across, Math.sin(angle) * radius);
		position.setXYZ(index, point.x, point.y, point.z);
	}
	position.needsUpdate = true;
	footprint.geometry.computeBoundingSphere();
	(footprint.material as THREE.LineBasicMaterial).color.set(
		String(beam.userData.stageBeamColor ?? "#ffffff"),
	);
}

function refreshGroundFootprints(scene: THREE.Scene) {
	scene.updateMatrixWorld(true);
	const footprints = new Map<string, THREE.LineLoop>();
	scene.traverse((object) => {
		if (
			object.name === "beam-ground-footprint" &&
			object instanceof THREE.LineLoop
		)
			footprints.set(String(object.userData.stageBeamSource), object);
	});
	scene.traverse((beam) => {
		if (beam.userData.stageDirectionalBeam !== true) return;
		const footprint = footprints.get(beam.uuid);
		if (footprint) updateGroundFootprint(footprint, beam);
	});
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
		if (object.userData.stageDirectionalBeam === true) beams.push(object);
	});
	for (const beam of beams) {
		const footprint = new THREE.LineLoop(
			new THREE.BufferGeometry().setAttribute(
				"position",
				new THREE.Float32BufferAttribute(49 * 3, 3),
			),
			new THREE.LineBasicMaterial({
				color: 0xffffff,
				transparent: true,
				opacity: 0.62,
			}),
		);
		footprint.name = "beam-ground-footprint";
		footprint.userData.stageBeamSource = beam.uuid;
		scene.add(footprint);
		updateGroundFootprint(footprint, beam);
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

export function applyStageVisualization(
	fixtures: Stage3dFixture[],
	snapshot: VisualizationSnapshot | null,
	fixtureObjects: Map<string, THREE.Object3D>,
	showBeamGuides: boolean,
	renderQuality: StageRenderQuality,
	virtualHighlight: Set<string> = new Set(),
) {
	const context = createSceneContext(
		snapshot,
		new Set(),
		showBeamGuides,
		virtualHighlight,
		renderQuality,
	);
	for (const item of fixtures) {
		const instanceId = item.instanceId ?? item.fixture.fixture_id;
		const root = fixtureObjects.get(instanceId);
		if (!root) continue;
		const fixtureId = item.fixture.fixture_id;
		const attributes = context.byFixture.get(fixtureId) ?? new Map();
		const mode = profileMode(item.fixture);
		if (mode) {
			updateFixtureProfileGeometry(root, {
				fixture: item.fixture,
				mode,
				byFixture: context.byFixture,
				selected: false,
				snapshot,
				projectedOwners: context.projectedOwners,
				showBeamGuides,
				renderQuality,
				virtualHighlight: virtualHighlight.has(fixtureId),
			});
		} else {
			updateFallbackFixture(
				root,
				item,
				attributes,
				fallbackRenderState(
					item,
					attributes,
					snapshot,
					virtualHighlight.has(fixtureId),
				),
				showBeamGuides,
				renderQuality,
			);
		}
	}
	const scene = fixtureObjects.values().next().value?.parent;
	if (scene instanceof THREE.Scene) refreshGroundFootprints(scene);
}

export function disposeScene(scene: THREE.Scene) {
	scene.traverse((object) => {
		const mesh = object as THREE.Mesh;
		if (!mesh.geometry?.userData.stageSharedModelResource)
			mesh.geometry?.dispose();
		const materials = Array.isArray(mesh.material)
			? mesh.material
			: mesh.material
				? [mesh.material]
				: [];
		for (const material of materials) {
			if (!material.userData.stageSharedModelResource) material.dispose();
		}
	});
}
