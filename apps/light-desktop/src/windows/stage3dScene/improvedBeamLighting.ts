import * as THREE from "three";
import type { StageRenderQuality } from "../../types";

export const MAX_IMPROVED_STAGE_LIGHTS = 8;
export const IMPROVED_STAGE_SHADOW_MAP_SIZE = 128;

const beamDirection = new THREE.Vector3(0, -1, 0);
const RETAINED_LIGHT_HYSTERESIS = 1.1;

function isOpaqueStageMesh(object: THREE.Object3D): object is THREE.Mesh {
	if (!(object instanceof THREE.Mesh) || !object.visible) return false;
	if (
		object.name === "light-emitting-surface" ||
		object.name === "beam-volume" ||
		object.name === "beam-core" ||
		object.name === "beam-improved-volume" ||
		object.name === "selection-outline"
	)
		return false;
	const materials = Array.isArray(object.material)
		? object.material
		: [object.material];
	return materials.some(
		(material) =>
			!material.transparent &&
			(typeof material.opacity !== "number" || material.opacity > 0),
	);
}

function fixtureRoot(object: THREE.Object3D) {
	let current: THREE.Object3D | null = object;
	while (current) {
		if (current.name.startsWith("fixture:")) return current;
		current = current.parent;
	}
	return null;
}

function isDescendantOf(object: THREE.Object3D, ancestor: THREE.Object3D) {
	let current: THREE.Object3D | null = object;
	while (current) {
		if (current === ancestor) return true;
		current = current.parent;
	}
	return false;
}

function improvedVolume(beam: THREE.Object3D) {
	const volume = beam.getObjectByName("beam-improved-volume");
	return volume instanceof THREE.Mesh ? volume : null;
}

function stableBeamIdentity(beam: THREE.Object3D) {
	const hierarchy: string[] = [];
	let current: THREE.Object3D | null = beam;
	while (current) {
		if (current.name) hierarchy.push(current.name);
		current = current.parent;
	}
	return [
		...hierarchy.reverse(),
		String(beam.userData.headId ?? ""),
		String(beam.userData.emitterId ?? ""),
	].join("/");
}

function contributingScore(beam: THREE.Object3D) {
	if (beam.userData.stageBeamActive !== true || !improvedVolume(beam)?.visible)
		return 0;
	const intensity = THREE.MathUtils.clamp(
		Number(beam.userData.stageBeamIntensity) || 0,
		0,
		1,
	);
	const radius = Math.max(0, Number(beam.userData.stageBeamRadius) || 0);
	const distance = Math.max(
		0.001,
		Number(beam.userData.stageBeamDistance) || 0.001,
	);
	return intensity * THREE.MathUtils.clamp(radius / distance, 0.02, 1);
}

function selectEnhancedBeams(
	scene: THREE.Scene,
	directionalBeams: THREE.Object3D[],
) {
	const retained = new Set<string>(
		Array.isArray(scene.userData.stageImprovedBeamBudget)
			? scene.userData.stageImprovedBeamBudget
			: [],
	);
	const ranked = directionalBeams
		.map((beam) => {
			const identity = stableBeamIdentity(beam);
			const score = contributingScore(beam);
			return {
				beam,
				identity,
				score,
				budgetScore:
					score * (retained.has(identity) ? RETAINED_LIGHT_HYSTERESIS : 1),
			};
		})
		.filter(({ score }) => score > 0)
		.sort(
			(left, right) =>
				right.budgetScore - left.budgetScore ||
				left.identity.localeCompare(right.identity),
		)
		.slice(0, MAX_IMPROVED_STAGE_LIGHTS);
	scene.userData.stageImprovedBeamBudget = ranked.map(
		({ identity }) => identity,
	);
	return new Set(ranked.map(({ beam }) => beam));
}

function setImprovedBeamLength(volume: THREE.Mesh, ratio: number) {
	const previous = Number(volume.userData.stageImprovedLengthRatio) || 1;
	const next = THREE.MathUtils.clamp(ratio, 0.01, 1);
	const change = next / previous;
	volume.scale.x *= change;
	volume.scale.y *= change;
	volume.scale.z *= change;
	volume.userData.stageImprovedLengthRatio = next;
}

function mixHashNumber(hash: number, value: number) {
	return (
		Math.imul(
			hash ^ (Number.isFinite(value) ? Math.round(value * 10_000) : 0),
			16_777_619,
		) >>> 0
	);
}

function mixHashString(hash: number, value: string) {
	let mixed = hash;
	for (let index = 0; index < value.length; index++)
		mixed = Math.imul(mixed ^ value.charCodeAt(index), 16_777_619) >>> 0;
	return mixed;
}

function shadowConfigurationSignature(
	enhanced: Set<THREE.Object3D>,
	opaqueMeshes: THREE.Mesh[],
) {
	let hash = 2_166_136_261;
	for (const beam of enhanced) {
		hash = mixHashString(hash, stableBeamIdentity(beam));
		for (const value of beam.matrixWorld.elements)
			hash = mixHashNumber(hash, value);
		const light = beam.getObjectByName("stage-improved-spotlight");
		if (light instanceof THREE.SpotLight) {
			hash = mixHashNumber(hash, light.angle);
			hash = mixHashNumber(hash, light.distance);
		}
	}
	for (const mesh of opaqueMeshes) {
		hash = mixHashString(hash, mesh.uuid);
		hash = mixHashString(hash, mesh.geometry.uuid);
		for (const value of mesh.matrixWorld.elements)
			hash = mixHashNumber(hash, value);
	}
	return hash;
}

function createSpotLight(beam: THREE.Object3D) {
	const light = new THREE.SpotLight(0xffffff, 0, 7, Math.PI / 6, 0.35, 1);
	light.name = "stage-improved-spotlight";
	light.position.set(0, 0, 0);
	light.castShadow = true;
	light.shadow.mapSize.set(
		IMPROVED_STAGE_SHADOW_MAP_SIZE,
		IMPROVED_STAGE_SHADOW_MAP_SIZE,
	);
	light.shadow.camera.near = 0.05;
	light.shadow.camera.far = 8;
	light.shadow.bias = -0.0005;
	const target = new THREE.Object3D();
	target.name = "stage-improved-spotlight-target";
	beam.add(light, target);
	light.target = target;
	return light;
}

function disposeSpotLight(beam: THREE.Object3D) {
	const light = beam.getObjectByName("stage-improved-spotlight");
	const target = beam.getObjectByName("stage-improved-spotlight-target");
	if (light instanceof THREE.SpotLight) {
		light.removeFromParent();
		light.dispose();
	}
	target?.removeFromParent();
}

function prepareOpaqueStageGeometry(scene: THREE.Scene) {
	const meshes: THREE.Mesh[] = [];
	scene.traverse((object) => {
		if (!isOpaqueStageMesh(object)) return;
		object.receiveShadow = true;
		object.castShadow = object.name !== "stage-floor";
		meshes.push(object);
	});
	return meshes;
}

function resetImprovedBeam(beam: THREE.Object3D) {
	const volume = improvedVolume(beam);
	if (volume) setImprovedBeamLength(volume, 1);
	disposeSpotLight(beam);
}

function updateImprovedBeam(
	beam: THREE.Object3D,
	opaqueMeshes: THREE.Mesh[],
	raycaster: THREE.Raycaster,
) {
	const volume = improvedVolume(beam);
	if (!volume) return;
	const active = beam.userData.stageBeamActive === true && volume.visible;
	const referenceDistance = Number(beam.userData.stageBeamDistance);
	const referenceRadius = Number(beam.userData.stageBeamRadius);
	if (
		!active ||
		!Number.isFinite(referenceDistance) ||
		!Number.isFinite(referenceRadius) ||
		referenceDistance <= 0
	) {
		setImprovedBeamLength(volume, 1);
		const light = beam.getObjectByName("stage-improved-spotlight");
		if (light instanceof THREE.SpotLight) light.visible = false;
		return;
	}

	const origin = beam.getWorldPosition(new THREE.Vector3());
	const direction = beamDirection
		.clone()
		.applyQuaternion(beam.getWorldQuaternion(new THREE.Quaternion()))
		.normalize();
	const ownFixture = fixtureRoot(beam);
	const occluders = ownFixture
		? opaqueMeshes.filter((mesh) => !isDescendantOf(mesh, ownFixture))
		: opaqueMeshes;
	raycaster.set(origin, direction);
	raycaster.near = 0.02;
	raycaster.far = referenceDistance;
	const hit = raycaster.intersectObjects(occluders, false)[0];
	const distance = Math.max(
		0.05,
		Math.min(referenceDistance, hit?.distance ?? referenceDistance),
	);
	setImprovedBeamLength(volume, distance / referenceDistance);

	const light =
		beam.getObjectByName("stage-improved-spotlight") instanceof THREE.SpotLight
			? (beam.getObjectByName("stage-improved-spotlight") as THREE.SpotLight)
			: createSpotLight(beam);
	const target = light.target;
	target.position.set(0, -distance, 0);
	light.visible = true;
	light.color.set(String(beam.userData.stageBeamColor ?? "#ffffff"));
	light.intensity =
		THREE.MathUtils.clamp(Number(beam.userData.stageBeamIntensity) || 0, 0, 1) *
		500;
	// The retained volume ends at the first surface, while the light must reach
	// slightly beyond it or physically based attenuation reaches zero exactly
	// where the surface should be illuminated.
	light.distance = Math.min(referenceDistance + 0.5, distance + 0.5);
	light.angle = THREE.MathUtils.clamp(
		Math.atan(referenceRadius / referenceDistance),
		THREE.MathUtils.degToRad(1),
		Math.PI / 2 - 0.01,
	);
	light.shadow.camera.far = Math.max(0.1, light.distance);
	light.shadow.camera.updateProjectionMatrix();
}

/**
 * Applies the capability-proven subset of richer beam rendering to a stable,
 * bounded set of directional emitters. Resolved intensity and beam coverage
 * rank contributors, stable fixture/head/emitter/source identity breaks ties,
 * and a retained-owner bonus prevents rapid budget churn. All emitters outside
 * the eight-light budget retain their feathered volume without adding light or
 * shadow resources.
 */
export function refreshImprovedBeamLighting(
	scene: THREE.Scene,
	renderQuality: StageRenderQuality,
) {
	const directionalBeams: THREE.Object3D[] = [];
	scene.updateMatrixWorld(true);
	scene.traverse((object) => {
		if (object.userData.stageDirectionalBeam === true)
			directionalBeams.push(object);
	});
	if (renderQuality !== "improved_beams") {
		scene.userData.stageImprovedBeamBudget = [];
		scene.userData.stageImprovedShadowSignature = null;
		scene.userData.stageImprovedShadowsDirty = false;
		for (const beam of directionalBeams) resetImprovedBeam(beam);
		return;
	}

	const enhanced = selectEnhancedBeams(scene, directionalBeams);
	for (const beam of directionalBeams) {
		if (!enhanced.has(beam)) resetImprovedBeam(beam);
	}
	const opaqueMeshes = prepareOpaqueStageGeometry(scene);
	const raycaster = new THREE.Raycaster();
	for (const beam of enhanced)
		updateImprovedBeam(beam, opaqueMeshes, raycaster);
	scene.updateMatrixWorld(true);
	const signature = shadowConfigurationSignature(enhanced, opaqueMeshes);
	scene.userData.stageImprovedShadowsDirty =
		scene.userData.stageImprovedShadowsDirty === true ||
		scene.userData.stageImprovedShadowSignature !== signature;
	scene.userData.stageImprovedShadowSignature = signature;
}

export function disposeImprovedBeamLighting(scene: THREE.Scene) {
	const beams: THREE.Object3D[] = [];
	scene.traverse((object) => {
		if (object.userData.stageDirectionalBeam === true) beams.push(object);
	});
	for (const beam of beams) disposeSpotLight(beam);
}
