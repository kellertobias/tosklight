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
import { drawsBeamLine, drawsBeamVolume } from "./renderStyle";

const footprintOrigin = new THREE.Vector3();
const footprintRotation = new THREE.Quaternion();
const footprintDirection = new THREE.Vector3();
const footprintTangent = new THREE.Vector3();
const footprintBitangent = new THREE.Vector3();
const footprintRay = new THREE.Vector3();
const footprintPoint = new THREE.Vector3();
const footprintUp = new THREE.Vector3(0, 1, 0);
const footprintSide = new THREE.Vector3(1, 0, 0);

function fixtureStructureKey(item: Stage3dFixture) {
	const definition = item.fixture.definition;
	return [
		definition.id,
		definition.revision,
		definition.profile_id ?? "",
		definition.mode_id ?? "",
		item.instanceId ?? item.fixture.fixture_id,
		JSON.stringify(item.installedAppearance ?? null),
		item.shaperAngle ?? "",
	].join("\u0000");
}

function fixtureTransformKey(item: Stage3dFixture) {
	return JSON.stringify([item.position, item.bracketAngle ?? 0]);
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
	const byFixture = physicalValues(item, context.byFixture);
	const attributes = byFixture.get(fixtureId) ?? new Map();
	const { root, instanceId } = createFixtureRoot(item, selected);
	const mode = profileMode(item.fixture);
	const profileGeometry =
		mode && usesProfileGeometry(item, mode)
			? buildFixtureProfileGeometry({
					fixture: item.fixture,
					mode,
					byFixture,
					selected,
					snapshot: context.snapshot,
					projectedOwners: context.projectedOwners,
					showBeamGuides: context.showBeamGuides,
					renderQuality: context.renderQuality,
					virtualHighlight: context.virtualHighlight.has(fixtureId),
					resources: context.resources,
					installedAppearance: item.installedAppearance,
					shaperAngle: item.shaperAngle,
				})
			: null;
	if (profileGeometry) {
		root.add(profileGeometry);
		root.userData.stageProfileGeometryRuntime =
			profileGeometry.userData.stageProfileGeometryRuntime;
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

function physicalValues(
	item: Stage3dFixture,
	source: StageSceneContext["byFixture"],
) {
	if (!item.invertPan && !item.invertTilt) return source;
	const result = new Map(source);
	const owners = [
		item.fixture.fixture_id,
		...(item.fixture.logical_heads ?? []).map((head) => head.fixture_id),
	];
	for (const owner of owners) {
		const current = source.get(owner);
		if (!current) continue;
		const values = new Map(current);
		for (const [attribute, invert] of [
			["pan", item.invertPan],
			["tilt", item.invertTilt],
		] as const) {
			const value = values.get(attribute);
			if (invert && value?.kind === "normalized")
				values.set(attribute, {
					kind: "normalized",
					value: 1 - Math.max(0, Math.min(1, value.value)),
				});
		}
		result.set(owner, values);
	}
	return result;
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
	const origin = footprintOrigin.setFromMatrixPosition(beam.matrixWorld);
	const rotation = footprintRotation.setFromRotationMatrix(beam.matrixWorld);
	const referenceRadius = Number(beam.userData.stageBeamRadius);
	const referenceDistance = Number(beam.userData.stageBeamDistance);
	const material = footprint.material as THREE.LineBasicMaterial;
	material.color.set(String(beam.userData.stageBeamColor ?? "#ffffff"));
	material.opacity = THREE.MathUtils.clamp(
		0.18 + Number(beam.userData.stageBeamIntensity ?? 0) * 0.5,
		0.18,
		0.68,
	);
	const geometrySignature = [
		origin.x,
		origin.y,
		origin.z,
		rotation.x,
		rotation.y,
		rotation.z,
		rotation.w,
		referenceRadius,
		referenceDistance,
	];
	const previousSignature = footprint.userData.stageGroundGeometrySignature as
		| number[]
		| undefined;
	if (
		previousSignature?.length === geometrySignature.length &&
		previousSignature.every(
			(value, index) => value === geometrySignature[index],
		)
	)
		return;
	footprint.userData.stageGroundGeometrySignature = geometrySignature;
	const direction = footprintDirection
		.set(0, -1, 0)
		.applyQuaternion(rotation)
		.normalize();
	if (direction.y >= -0.001) {
		footprint.visible = false;
		return;
	}
	if (
		!Number.isFinite(referenceRadius) ||
		!Number.isFinite(referenceDistance) ||
		referenceDistance <= 0
	) {
		footprint.visible = false;
		return;
	}
	const halfAngle = Math.atan(referenceRadius / referenceDistance);
	const tangent = footprintTangent
		.crossVectors(
			direction,
			Math.abs(direction.y) < 0.99 ? footprintUp : footprintSide,
		)
		.normalize();
	const bitangent = footprintBitangent
		.crossVectors(direction, tangent)
		.normalize();
	const position = footprint.geometry.getAttribute(
		"position",
	) as THREE.BufferAttribute;
	for (let index = 0; index < 49; index++) {
		const angle = (index / 48) * Math.PI * 2;
		const ray = footprintRay
			.copy(direction)
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
		const point = footprintPoint.copy(origin).addScaledVector(ray, rayDistance);
		position.setXYZ(index, point.x, groundReferenceY, point.z);
	}
	position.needsUpdate = true;
	footprint.geometry.computeBoundingSphere();
}

function refreshGroundFootprints(
	scene: THREE.Scene,
	changedFixtureIds?: ReadonlySet<string>,
	renderQuality: StageRenderQuality = "lines_and_beams",
) {
	const footprints = scene.userData.stageGroundFootprints as
		| Map<string, THREE.LineLoop>
		| undefined;
	const beams = scene.userData.stageDirectionalBeams as
		| THREE.Object3D[]
		| undefined;
	if (!footprints || !beams) return;
	if (!drawsBeamLine(renderQuality)) {
		for (const footprint of footprints.values()) footprint.visible = false;
		return;
	}
	scene.updateMatrixWorld(true);
	for (const beam of beams) {
		const fixtureId = beam.userData.stageFixtureId as string | undefined;
		if (changedFixtureIds && fixtureId && !changedFixtureIds.has(fixtureId))
			continue;
		const footprint = footprints.get(beam.uuid);
		if (footprint) updateGroundFootprint(footprint, beam);
	}
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
	const footprints = new Map<string, THREE.LineLoop>();
	for (const beam of beams) {
		let fixtureRoot: THREE.Object3D | null = beam;
		while (fixtureRoot && typeof fixtureRoot.userData.fixtureId !== "string")
			fixtureRoot = fixtureRoot.parent;
		beam.userData.stageFixtureId = fixtureRoot?.userData.fixtureId;
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
		footprints.set(beam.uuid, footprint);
		updateGroundFootprint(footprint, beam);
		footprint.visible =
			footprint.visible &&
			drawsBeamLine(renderQuality);
	}
	scene.userData.stageDirectionalBeams = beams;
	scene.userData.stageGroundFootprints = footprints;
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
	scene.userData.stageDirectionalBeams = undefined;
	scene.userData.stageGroundFootprints = undefined;
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
	} else refreshGroundFootprints(scene, undefined, renderQuality);
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
	changedFixtureIds?: ReadonlySet<string>,
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
	const updatedFixtureIds = changedFixtureIds ? new Set<string>() : undefined;
	for (const item of fixtures) {
		const fixtureId = item.fixture.fixture_id;
		if (
			changedFixtureIds &&
			!changedFixtureIds.has(fixtureId) &&
			!item.fixture.logical_heads.some((head) =>
				changedFixtureIds.has(head.fixture_id),
			)
		)
			continue;
		updatedFixtureIds?.add(fixtureId);
		const instanceId = item.instanceId ?? item.fixture.fixture_id;
		const root = fixtureObjects.get(instanceId);
		if (!root) continue;
		setSelectionOutlineVisibility(
			root,
			showSelection && selected.has(fixtureId),
		);
		const byFixture = physicalValues(item, context.byFixture);
		const attributes = byFixture.get(fixtureId) ?? new Map();
		const mode = profileMode(item.fixture);
		if (mode && usesProfileGeometry(item, mode)) {
			updateFixtureProfileGeometry(root, {
				fixture: item.fixture,
				mode,
				byFixture,
				selected: false,
				snapshot,
				projectedOwners: context.projectedOwners,
				showBeamGuides,
				renderQuality,
				virtualHighlight: virtualHighlight.has(fixtureId),
				resources: context.resources,
				installedAppearance: item.installedAppearance,
				shaperAngle: item.shaperAngle,
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
	refreshGroundFootprints(scene, updatedFixtureIds, renderQuality);
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
