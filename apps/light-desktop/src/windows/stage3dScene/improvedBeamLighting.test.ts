import * as THREE from "three";
import { describe, expect, it } from "vitest";
import {
	IMPROVED_STAGE_SHADOW_MAP_SIZE,
	MAX_IMPROVED_STAGE_LIGHTS,
	refreshImprovedBeamLighting,
} from "./improvedBeamLighting";

function directionalBeam(index: number) {
	const root = new THREE.Group();
	root.name = `fixture:${index}:${index}`;
	root.position.set(index * 0.25, 2, 0);
	const beam = new THREE.Group();
	beam.name = `beam:${index}`;
	beam.userData.stageDirectionalBeam = true;
	beam.userData.stageBeamActive = true;
	beam.userData.stageBeamIntensity = 0.75;
	beam.userData.stageBeamColor = "#ff8040";
	beam.userData.stageBeamRadius = 1;
	beam.userData.stageBeamDistance = 7;
	const volume = new THREE.Mesh(
		new THREE.ConeGeometry(1, 7, 8, 1, true).translate(0, -3.5, 0),
		new THREE.MeshBasicMaterial(),
	);
	volume.name = "beam-improved-volume";
	root.add(beam);
	beam.add(volume);
	return { root, beam, volume };
}

function floor() {
	const floor = new THREE.Mesh(
		new THREE.PlaneGeometry(20, 20),
		new THREE.MeshStandardMaterial(),
	);
	floor.name = "stage-floor";
	floor.rotation.x = -Math.PI / 2;
	return floor;
}

describe("Improved Stage beam lighting", () => {
	it("shortens the retained feathered volume at the first opaque hit", () => {
		const scene = new THREE.Scene();
		const stageFloor = floor();
		const occluder = new THREE.Mesh(
			new THREE.BoxGeometry(0.2, 0.2, 0.2),
			new THREE.MeshStandardMaterial(),
		);
		occluder.position.y = 1;
		const { root, beam, volume } = directionalBeam(0);
		scene.add(stageFloor, occluder, root);

		refreshImprovedBeamLighting(scene, "improved_beams");

		expect(volume.scale.y).toBeCloseTo(0.1286, 3);
		expect(stageFloor.receiveShadow).toBe(true);
		expect(stageFloor.castShadow).toBe(false);
		expect(occluder.receiveShadow).toBe(true);
		expect(occluder.castShadow).toBe(true);
		const light = beam.getObjectByName("stage-improved-spotlight");
		expect(light).toBeInstanceOf(THREE.SpotLight);
		expect((light as THREE.SpotLight).position.toArray()).toEqual([0, 0, 0]);
		expect(
			(light as THREE.SpotLight)
				.getWorldPosition(new THREE.Vector3())
				.distanceTo(beam.getWorldPosition(new THREE.Vector3())),
		).toBeCloseTo(0);
		expect((light as THREE.SpotLight).intensity).toBe(375);
		expect((light as THREE.SpotLight).shadow.mapSize.width).toBe(
			IMPROVED_STAGE_SHADOW_MAP_SIZE,
		);
		expect((light as THREE.SpotLight).distance).toBeCloseTo(1.4, 3);
	});

	it("keeps the richer resources capped and stable in retained scene order", () => {
		const scene = new THREE.Scene();
		scene.add(floor());
		const beams = Array.from({ length: 12 }, (_, index) =>
			directionalBeam(index),
		);
		for (const { root } of beams) scene.add(root);

		refreshImprovedBeamLighting(scene, "improved_beams");

		expect(
			beams.filter(({ beam }) =>
				beam.getObjectByName("stage-improved-spotlight"),
			),
		).toHaveLength(MAX_IMPROVED_STAGE_LIGHTS);
		const retained = beams[0].beam.getObjectByName("stage-improved-spotlight");
		refreshImprovedBeamLighting(scene, "improved_beams");
		expect(beams[0].beam.getObjectByName("stage-improved-spotlight")).toBe(
			retained,
		);
	});

	it("selects higher contributors while hysteresis prevents marginal churn", () => {
		const scene = new THREE.Scene();
		scene.add(floor());
		const beams = Array.from({ length: 9 }, (_, index) =>
			directionalBeam(index),
		);
		for (const { root } of beams) scene.add(root);
		beams[8].beam.userData.stageBeamIntensity = 0.1;
		refreshImprovedBeamLighting(scene, "improved_beams");
		expect(
			beams[8].beam.getObjectByName("stage-improved-spotlight"),
		).toBeUndefined();

		beams[8].beam.userData.stageBeamIntensity = 0.8;
		refreshImprovedBeamLighting(scene, "improved_beams");
		expect(
			beams[8].beam.getObjectByName("stage-improved-spotlight"),
		).toBeUndefined();

		beams[8].beam.userData.stageBeamIntensity = 1;
		refreshImprovedBeamLighting(scene, "improved_beams");
		expect(
			beams[8].beam.getObjectByName("stage-improved-spotlight"),
		).toBeInstanceOf(THREE.SpotLight);
	});

	it("reuses shadow depth for intensity-only changes and invalidates transforms", () => {
		const scene = new THREE.Scene();
		scene.add(floor());
		const { root, beam } = directionalBeam(0);
		scene.add(root);
		refreshImprovedBeamLighting(scene, "improved_beams");
		expect(scene.userData.stageImprovedShadowsDirty).toBe(true);
		scene.userData.stageImprovedShadowsDirty = false;

		beam.userData.stageBeamIntensity = 0.5;
		refreshImprovedBeamLighting(scene, "improved_beams");
		expect(scene.userData.stageImprovedShadowsDirty).toBe(false);
		expect(
			(beam.getObjectByName("stage-improved-spotlight") as THREE.SpotLight)
				.intensity,
		).toBe(250);

		root.position.x += 0.1;
		refreshImprovedBeamLighting(scene, "improved_beams");
		expect(scene.userData.stageImprovedShadowsDirty).toBe(true);
	});

	it("releases lighting resources and restores beam length in other qualities", () => {
		const scene = new THREE.Scene();
		scene.add(floor());
		const { root, beam, volume } = directionalBeam(0);
		scene.add(root);
		refreshImprovedBeamLighting(scene, "improved_beams");
		expect(volume.scale.y).toBeLessThan(1);

		refreshImprovedBeamLighting(scene, "lines_and_beams");

		expect(volume.scale.y).toBeCloseTo(1);
		expect(beam.getObjectByName("stage-improved-spotlight")).toBeUndefined();
	});
});
