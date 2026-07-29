import * as THREE from "three";
import type { VisualizationSnapshot } from "../../api/types";
import type { StageRenderQuality } from "../../types";
import { profileMode, valuesByFixture } from "./attributeValues";
import {
	applyFixtureRootTransform,
	createFixtureRoot,
	fallbackRenderState,
	mountFallbackFixture,
	updateFallbackFixture,
} from "./fallbackFixture";
import {
	disposeImprovedBeamLighting,
	refreshImprovedBeamLighting,
} from "./improvedBeamLighting";
import {
	buildFixtureProfileGeometry,
	updateFixtureProfileGeometry,
} from "./profileGeometry";
import { StageProceduralResourceCache } from "./resources";
import { setSelectionOutlineVisibility } from "./sceneObjects";
import type { Stage3dFixture, StageSceneContext } from "./types";

function fixtureStructureKey(item: Stage3dFixture) {
	return JSON.stringify([
		item.fixture.definition,
		item.fixture.logical_heads,
		item.instanceId ?? item.fixture.fixture_id,
	]);
}

function fixtureTransformKey(item: Stage3dFixture) {
	return JSON.stringify(item.position);
}

function usesProfileGeometry(
	item: Stage3dFixture,
	mode: NonNullable<ReturnType<typeof profileMode>>,
) {
	return Boolean(
		mode.geometry.nodes.length &&
			(mode.geometry.emitters.length ||
				item.fixture.definition.profile_snapshot?.patch_policy ===
					"visual_only"),
	);
}

function buildStageFixture(item: Stage3dFixture, context: StageSceneContext) {
	const fixtureId = item.fixture.fixture_id;
	const selected = context.selected.has(fixtureId);
	const attributes = context.byFixture.get(fixtureId) ?? new Map();
	const { root, instanceId } = createFixtureRoot(item, selected);
	const mode = profileMode(item.fixture);
	const profileGeometry =
		mode && usesProfileGeometry(item, mode)
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
					resources: context.resources,
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
			context.resources,
		);
	}
	root.userData.stageFixtureStructureKey = fixtureStructureKey(item);
	root.userData.stageFixtureTransformKey = fixtureTransformKey(item);
	return { root, instanceId };
}

function addStageFloor(scene: THREE.Scene) {
	const floor = new THREE.Mesh(
		new THREE.PlaneGeometry(12, 8),
		new THREE.MeshStandardMaterial({ color: 0x151b20, roughness: 0.9 }),
	);
	floor.name = "stage-floor";
	floor.receiveShadow = true;
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
	resources: StageProceduralResourceCache,
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
		resources,
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
	const halfAngle = Math.atan(referenceRadius / referenceDistance);
	const tangent = new THREE.Vector3()
		.crossVectors(
			direction,
			Math.abs(direction.y) < 0.99
				? new THREE.Vector3(0, 1, 0)
				: new THREE.Vector3(1, 0, 0),
		)
		.normalize();
	const bitangent = new THREE.Vector3()
		.crossVectors(direction, tangent)
		.normalize();
	const position = footprint.geometry.getAttribute(
		"position",
	) as THREE.BufferAttribute;
	for (let index = 0; index < 49; index++) {
		const angle = (index / 48) * Math.PI * 2;
		const ray = direction
			.clone()
			.multiplyScalar(Math.cos(halfAngle))
			.addScaledVector(tangent, Math.cos(angle) * Math.sin(halfAngle))
			.addScaledVector(bitangent, Math.sin(angle) * Math.sin(halfAngle))
			.normalize();
		// A cone edge that is parallel to or aimed away from the ground has no
		// finite closed footprint. Keep the center line, but do not invent one.
		if (ray.y >= -0.001) {
			footprint.visible = false;
			return;
		}
		const groundReferenceY = 0.006;
		const rayDistance = (groundReferenceY - origin.y) / ray.y;
		if (rayDistance <= 0) {
			footprint.visible = false;
			return;
		}
		const point = origin.clone().addScaledVector(ray, rayDistance);
		position.setXYZ(index, point.x, groundReferenceY, point.z);
	}
	position.needsUpdate = true;
	footprint.geometry.computeBoundingSphere();
	const material = footprint.material as THREE.LineBasicMaterial;
	material.color.set(String(beam.userData.stageBeamColor ?? "#ffffff"));
	material.opacity = THREE.MathUtils.clamp(
		0.18 + Number(beam.userData.stageBeamIntensity ?? 0) * 0.5,
		0.18,
		0.68,
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
		footprint.visible =
			footprint.visible &&
			(renderQuality === "lines_only" || renderQuality === "lines_and_beams");
	}
}

function removeGroundFootprints(scene: THREE.Scene) {
	const footprints: THREE.Object3D[] = [];
	scene.traverse((object) => {
		if (object.name === "beam-ground-footprint") footprints.push(object);
	});
	for (const footprint of footprints) {
		footprint.removeFromParent();
		disposeObjectResources(footprint);
	}
}

export function reconcileStageFixtures(
	scene: THREE.Scene,
	fixtureObjects: Map<string, THREE.Object3D>,
	fixtures: Stage3dFixture[],
	snapshot: VisualizationSnapshot | null,
	selected: Set<string>,
	showBeamGuides: boolean,
	virtualHighlight: Set<string>,
	renderQuality: StageRenderQuality,
	resources: StageProceduralResourceCache,
) {
	const context = createSceneContext(
		snapshot,
		selected,
		showBeamGuides,
		virtualHighlight,
		renderQuality,
		resources,
	);
	const nextIds = new Set(
		fixtures.map((item) => item.instanceId ?? item.fixture.fixture_id),
	);
	const removedInstanceIds: string[] = [];
	const changedFixtures: Stage3dFixture[] = [];
	let directionalStructureChanged = false;
	for (const [instanceId, root] of fixtureObjects) {
		if (nextIds.has(instanceId)) continue;
		fixtureObjects.delete(instanceId);
		root.removeFromParent();
		disposeObjectResources(root);
		removedInstanceIds.push(instanceId);
		directionalStructureChanged = true;
	}
	for (const item of fixtures) {
		const instanceId = item.instanceId ?? item.fixture.fixture_id;
		const existing = fixtureObjects.get(instanceId);
		const structureKey = fixtureStructureKey(item);
		if (
			existing &&
			existing.userData.stageFixtureStructureKey === structureKey
		) {
			const transformKey = fixtureTransformKey(item);
			if (existing.userData.stageFixtureTransformKey !== transformKey) {
				applyFixtureRootTransform(existing, item);
				existing.userData.stageFixtureTransformKey = transformKey;
			}
			continue;
		}
		if (existing) {
			existing.removeFromParent();
			disposeObjectResources(existing);
			fixtureObjects.delete(instanceId);
			removedInstanceIds.push(instanceId);
			directionalStructureChanged = true;
		}
		const { root } = buildStageFixture(item, context);
		scene.add(root);
		fixtureObjects.set(instanceId, root);
		changedFixtures.push(item);
		directionalStructureChanged = true;
	}
	if (directionalStructureChanged) {
		removeGroundFootprints(scene);
		addGroundFootprints(scene, renderQuality);
	} else refreshGroundFootprints(scene);
	refreshImprovedBeamLighting(scene, renderQuality);
	return { changedFixtures, removedInstanceIds };
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
	resources?: StageProceduralResourceCache,
) {
	const scene = createStageEnvironment(environmentBrightness, showFloorGrid);
	const stageResources = resources ?? new StageProceduralResourceCache();
	scene.userData.stageProceduralResources = stageResources;
	scene.userData.stageOwnsProceduralResources = !resources;
	const context = createSceneContext(
		snapshot,
		selected,
		showBeamGuides,
		virtualHighlight,
		renderQuality,
		stageResources,
	);
	const fixtureObjects = new Map<string, THREE.Object3D>();
	for (const item of fixtures) {
		const { root, instanceId } = buildStageFixture(item, context);
		scene.add(root);
		fixtureObjects.set(instanceId, root);
	}
	addGroundFootprints(scene, renderQuality);
	refreshImprovedBeamLighting(scene, renderQuality);
	return { scene, fixtureObjects };
}

export function applyStageVisualization(
	fixtures: Stage3dFixture[],
	snapshot: VisualizationSnapshot | null,
	fixtureObjects: Map<string, THREE.Object3D>,
	showBeamGuides: boolean,
	renderQuality: StageRenderQuality,
	virtualHighlight: Set<string> = new Set(),
	selected: Set<string> = new Set(),
	showSelection = false,
) {
	const scene =
		fixtureObjects.values().next().value?.parent instanceof THREE.Scene
			? fixtureObjects.values().next().value?.parent
			: null;
	if (!(scene instanceof THREE.Scene)) return;
	const resources = scene.userData
		.stageProceduralResources as StageProceduralResourceCache;
	const context = createSceneContext(
		snapshot,
		new Set(),
		showBeamGuides,
		virtualHighlight,
		renderQuality,
		resources,
	);
	for (const item of fixtures) {
		const instanceId = item.instanceId ?? item.fixture.fixture_id;
		const root = fixtureObjects.get(instanceId);
		if (!root) continue;
		const fixtureId = item.fixture.fixture_id;
		setSelectionOutlineVisibility(
			root,
			showSelection && selected.has(fixtureId),
		);
		const attributes = context.byFixture.get(fixtureId) ?? new Map();
		const mode = profileMode(item.fixture);
		if (mode && usesProfileGeometry(item, mode)) {
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
				resources: context.resources,
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
				context.resources,
			);
		}
	}
	refreshGroundFootprints(scene);
	scene.traverse((object) => {
		if (object.name === "beam-ground-footprint")
			object.visible =
				object.visible &&
				(renderQuality === "lines_only" || renderQuality === "lines_and_beams");
	});
	refreshImprovedBeamLighting(scene, renderQuality);
}

export function disposeObjectResources(object: THREE.Object3D) {
	object.traverse((object) => {
		if (object instanceof THREE.SpotLight) object.dispose();
		const mesh = object as THREE.Mesh;
		if (
			!mesh.geometry?.userData.stageSharedModelResource &&
			!mesh.geometry?.userData.stageSharedProceduralResource
		)
			mesh.geometry?.dispose();
		const materials = Array.isArray(mesh.material)
			? mesh.material
			: mesh.material
				? [mesh.material]
				: [];
		for (const material of materials) {
			if (
				!material.userData.stageSharedModelResource &&
				!material.userData.stageSharedProceduralResource
			)
				material.dispose();
		}
	});
}

export function disposeScene(scene: THREE.Scene) {
	disposeImprovedBeamLighting(scene);
	disposeObjectResources(scene);
	if (scene.userData.stageOwnsProceduralResources)
		(
			scene.userData.stageProceduralResources as
				| StageProceduralResourceCache
				| undefined
		)?.dispose();
}
