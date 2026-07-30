import * as THREE from "three";
import { describe, expect, it, vi } from "vitest";
import type { PatchedFixture, VisualizationSnapshot } from "../api/types";
import {
	blankChannel,
	blankFixtureProfile,
	blankHead,
	fixtureDefinitionFromProfileMode,
	geometryTemplate,
} from "../components/setup/fixtureProfileModel";
import {
	createBuiltInFixtureModel,
	inferBuiltInFixtureKind,
	movingLightTiltRadians,
} from "./builtInStageModels";
import {
	applyStageVisualization,
	buildStageScene,
	cueVisualization,
	disposeScene,
	fallbackEmitterIsDirectional,
	migrateStagePosition,
	mountFixtureModel,
	reconcileStageFixtures,
} from "./stage3dScene";
import { StageProceduralResourceCache } from "./stage3dScene/resources";

const fixture = (device_type: string, name: string) =>
	({
		fixture_id: "fixture",
		universe: 1,
		address: 1,
		definition: { device_type, name, manufacturer: "", model: name },
	}) as PatchedFixture;

describe("3D stage presentation and cue state", () => {
	it("applies routine fallback values without replacing retained scene resources", () => {
		const mover = fixture("moving wash", "A7 LED Wash");
		const fixtures = [
			{
				fixture: mover,
				index: 0,
				position: {
					x: 0,
					y: 0,
					z: 3,
					rotationX: 0,
					rotationY: 0,
					rotationZ: 0,
				},
			},
		];
		const snapshot = (
			intensity: number,
			pan: number,
			tilt: number,
			zoom: number,
		): VisualizationSnapshot => ({
			revision: 1,
			generated_at: "",
			grand_master: 1,
			blackout: false,
			values: [
				{
					fixture_id: "fixture",
					attribute: "intensity",
					value: { kind: "normalized", value: intensity },
				},
				{
					fixture_id: "fixture",
					attribute: "pan",
					value: { kind: "normalized", value: pan },
				},
				{
					fixture_id: "fixture",
					attribute: "tilt",
					value: { kind: "normalized", value: tilt },
				},
				{
					fixture_id: "fixture",
					attribute: "zoom",
					value: { kind: "normalized", value: zoom },
				},
			],
		});
		const built = buildStageScene(fixtures, snapshot(0.2, 0.25, 0.25, 0.2));
		const scene = built.scene;
		const root = built.fixtureObjects.get("fixture");
		const yoke = scene.getObjectByName("centered-rotating-yoke");
		const volume = scene.getObjectByName("beam-volume") as THREE.Mesh;
		const material = volume.material as THREE.MeshBasicMaterial;
		const sourceMaterial = (
			scene.getObjectByName("light-emitting-surface") as THREE.Mesh<
				THREE.BufferGeometry,
				THREE.Material
			>
		).material;
		const geometry = volume.geometry;
		const footprint = scene.getObjectByName("beam-ground-footprint");
		const previousOpacity = material.opacity;
		const previousMaterialVersion = material.version;
		const previousSourceMaterialVersion = sourceMaterial.version;

		applyStageVisualization(
			fixtures,
			snapshot(0.8, 0.75, 0.75, 0.8),
			built.fixtureObjects,
			true,
			"lines_and_beams",
		);

		expect(built.scene).toBe(scene);
		expect(built.fixtureObjects.get("fixture")).toBe(root);
		expect(scene.getObjectByName("centered-rotating-yoke")).toBe(yoke);
		expect(scene.getObjectByName("beam-volume")).toBe(volume);
		expect(volume.geometry).toBe(geometry);
		expect(volume.material).toBe(material);
		expect(scene.getObjectByName("beam-ground-footprint")).toBe(footprint);
		expect(material.opacity).toBeGreaterThan(previousOpacity);
		expect(material.version).toBe(previousMaterialVersion);
		expect(sourceMaterial.version).toBe(previousSourceMaterialVersion);
		expect(yoke?.rotation.y).toBeCloseTo(Math.PI / 2);
	});

	it("can omit the floor plane and grid from the scene", () => {
		const visible = buildStageScene([], null);
		expect(visible.scene.getObjectByName("stage-floor")).toBeTruthy();
		expect(visible.scene.getObjectByName("stage-floor-grid")).toBeTruthy();

		const hidden = buildStageScene([], null, new Set(), 1, false);
		expect(hidden.scene.getObjectByName("stage-floor")).toBeUndefined();
		expect(hidden.scene.getObjectByName("stage-floor-grid")).toBeUndefined();
	});

	it("creates selection outlines only while a fixture is selected", () => {
		const mover = fixture("moving wash", "A7 LED Wash");
		const fixtures = [
			{
				fixture: mover,
				index: 0,
				position: {
					x: 0,
					y: 0,
					z: 3,
					rotationX: 0,
					rotationY: 0,
					rotationZ: 0,
				},
			},
		];
		const built = buildStageScene(fixtures, null);
		expect(built.scene.getObjectByName("selection-outline")).toBeUndefined();

		applyStageVisualization(
			fixtures,
			null,
			built.fixtureObjects,
			true,
			"lines_and_beams",
			new Set(),
			new Set(["fixture"]),
			true,
		);
		expect(built.scene.getObjectByName("selection-outline")).toBeTruthy();

		applyStageVisualization(
			fixtures,
			null,
			built.fixtureObjects,
			true,
			"lines_and_beams",
		);
		expect(built.scene.getObjectByName("selection-outline")).toBeUndefined();
	});

	it("reconciles only the affected fixture root and updates transforms in place", () => {
		const firstFixture = {
			...fixture("moving wash", "First"),
			fixture_id: "first",
			definition: {
				...fixture("moving wash", "First").definition,
				id: "first-profile",
				revision: 1,
			},
		};
		const secondFixture = {
			...fixture("moving wash", "Second"),
			fixture_id: "second",
			definition: {
				...fixture("moving wash", "Second").definition,
				id: "second-profile",
				revision: 1,
			},
		};
		const item = (patched: PatchedFixture, x: number) => ({
			fixture: patched,
			index: 0,
			position: {
				x,
				y: 0,
				z: 3,
				rotationX: 0,
				rotationY: 0,
				rotationZ: 0,
			},
		});
		const initial = [item(firstFixture, 0), item(secondFixture, 2)];
		const built = buildStageScene(initial, null);
		const firstRoot = built.fixtureObjects.get("first");
		const secondRoot = built.fixtureObjects.get("second");
		const resources = built.scene.userData
			.stageProceduralResources as StageProceduralResourceCache;

		reconcileStageFixtures(
			built.scene,
			built.fixtureObjects,
			[item(firstFixture, 1), item(secondFixture, 2)],
			null,
			new Set(),
			true,
			new Set(),
			"lines_and_beams",
			resources,
		);
		expect(built.fixtureObjects.get("first")).toBe(firstRoot);
		expect(firstRoot?.position.x).toBe(1);
		expect(built.fixtureObjects.get("second")).toBe(secondRoot);

		const operationallyUpdatedFirst = {
			...firstFixture,
			name: "Updated operator label",
			universe: 12,
			address: 101,
			definition: {
				...firstFixture.definition,
				name: "Equivalent projection copy",
			},
		};
		const operationalResult = reconcileStageFixtures(
			built.scene,
			built.fixtureObjects,
			[item(operationallyUpdatedFirst, 1), item(secondFixture, 2)],
			null,
			new Set(),
			true,
			new Set(),
			"lines_and_beams",
			resources,
		);
		expect(operationalResult.changedFixtures).toHaveLength(0);
		expect(built.fixtureObjects.get("first")).toBe(firstRoot);

		const revisedFirst = {
			...firstFixture,
			definition: { ...firstFixture.definition, revision: 2 },
		};
		const result = reconcileStageFixtures(
			built.scene,
			built.fixtureObjects,
			[item(revisedFirst, 1), item(secondFixture, 2)],
			null,
			new Set(),
			true,
			new Set(),
			"lines_and_beams",
			resources,
		);
		expect(result.changedFixtures).toHaveLength(1);
		expect(built.fixtureObjects.get("first")).not.toBe(firstRoot);
		expect(built.fixtureObjects.get("second")).toBe(secondRoot);
	});

	it("reuses surface-owned beam geometry without disposing it during a scene swap", () => {
		const resources = new StageProceduralResourceCache();
		const mover = fixture("moving wash", "A7 LED Wash");
		const stageFixture = {
			fixture: mover,
			index: 0,
			position: {
				x: 0,
				y: 0,
				z: 3,
				rotationX: 0,
				rotationY: 0,
				rotationZ: 0,
			},
		};
		const first = buildStageScene(
			[stageFixture],
			null,
			new Set(),
			1,
			true,
			true,
			new Set(),
			"lines_and_beams",
			resources,
		);
		const second = buildStageScene(
			[{ ...stageFixture, instanceId: "second" }],
			null,
			new Set(),
			1,
			true,
			true,
			new Set(),
			"lines_and_beams",
			resources,
		);
		const firstGeometry = (
			first.scene.getObjectByName("beam-volume") as THREE.Mesh
		).geometry;
		const secondGeometry = (
			second.scene.getObjectByName("beam-volume") as THREE.Mesh
		).geometry;
		const dispose = vi.spyOn(firstGeometry, "dispose");
		expect(secondGeometry).toBe(firstGeometry);

		disposeScene(first.scene);
		expect(dispose).not.toHaveBeenCalled();
		resources.dispose();
		expect(dispose).toHaveBeenCalledOnce();
	});

	it("migrates legacy percentage positions into the meter-based stage", () => {
		expect(migrateStagePosition({ x: 50, y: 25, rotation: 90 }, 0)).toEqual({
			x: 0,
			y: 2,
			z: 5,
			rotationX: 0,
			rotationY: 0,
			rotationZ: 90,
		});
	});

	it("tracks cue values and explicit releases for thumbnails", () => {
		const base: VisualizationSnapshot = {
			revision: 1,
			generated_at: "",
			grand_master: 0.5,
			blackout: true,
			values: [],
		};
		const first = cueVisualization(base, [
			{
				fixture_id: "one",
				attribute: "intensity",
				value: { kind: "normalized", value: 0.8 },
			},
		]);
		expect(first.blackout).toBe(false);
		expect(first.grand_master).toBe(1);
		expect(first.values).toHaveLength(1);
		const released = cueVisualization(first, [
			{ fixture_id: "one", attribute: "intensity", value: null },
		]);
		expect(released.values).toHaveLength(0);
	});
});

describe("schema-v2 hierarchy and logical-head rendering", () => {
	it("consumes schema-v2 hierarchy motion, logical-head values, multiple emitters, and source layouts", () => {
		const profile = blankFixtureProfile();
		profile.manufacturer = "Acme";
		profile.name = "Twin Beam";
		profile.revision = 1;
		const mode = profile.modes[0];
		const second = { ...blankHead(1), master_shared: false };
		mode.heads.push(second);
		mode.channels = [
			{
				...blankChannel(mode),
				head_id: mode.heads[0].id,
				attribute: "intensity",
			},
			{ ...blankChannel(mode), head_id: second.id, attribute: "intensity" },
		];
		mode.geometry = geometryTemplate(
			"shared_pan_multi_head",
			mode.heads.map((head) => head.id),
		);
		mode.geometry.emitters[0].layout = {
			type: "matrix",
			columns: 2,
			rows: 2,
			spacing: { x: 40, y: 40, z: 0 },
		};
		mode.geometry.emitters[0].feather = 0.35;
		mode.geometry.emitters[0].focus = 0.7;
		const definition = fixtureDefinitionFromProfileMode(profile, mode);
		const fixture = {
			fixture_id: profile.id,
			universe: 1,
			address: 1,
			definition,
			logical_heads: [{ head_index: 1, fixture_id: "head-two" }],
		} as PatchedFixture;
		const snapshot: VisualizationSnapshot = {
			revision: 1,
			generated_at: "",
			grand_master: 1,
			blackout: false,
			values: [
				{
					fixture_id: profile.id,
					attribute: "pan",
					value: { kind: "normalized", value: 0.75 },
				},
				{
					fixture_id: profile.id,
					attribute: "tilt",
					value: { kind: "normalized", value: 0.25 },
				},
				{
					fixture_id: profile.id,
					attribute: "intensity",
					value: { kind: "normalized", value: 0.4 },
				},
				{
					fixture_id: profile.id,
					attribute: "beam.focus",
					value: { kind: "normalized", value: 0.2 },
				},
				{
					fixture_id: profile.id,
					attribute: "beam.zoom",
					value: { kind: "normalized", value: 0.75 },
				},
				{
					fixture_id: "head-two",
					attribute: "tilt",
					value: { kind: "normalized", value: 0.75 },
				},
				{
					fixture_id: "head-two",
					attribute: "intensity",
					value: { kind: "normalized", value: 0.8 },
				},
			],
		};
		const { scene } = buildStageScene(
			[
				{
					fixture,
					index: 0,
					position: {
						x: 0,
						y: 0,
						z: 3,
						rotationX: 0,
						rotationY: 0,
						rotationZ: 0,
					},
				},
			],
			snapshot,
		);
		const pan = mode.geometry.nodes.find(
			(node) => node.motion?.attribute === "pan",
		)!;
		const tilts = mode.geometry.nodes.filter(
			(node) => node.motion?.attribute === "tilt",
		);
		expect(
			scene.getObjectByName(`geometry-node:${pan.id}`)?.rotation.y,
		).toBeCloseTo(THREE.MathUtils.degToRad(135));
		expect(
			scene.getObjectByName(`geometry-node:${tilts[0].id}`)?.rotation.x,
		).toBeCloseTo(THREE.MathUtils.degToRad(-67.5));
		expect(
			scene.getObjectByName(`geometry-node:${tilts[1].id}`)?.rotation.x,
		).toBeCloseTo(THREE.MathUtils.degToRad(67.5));
		const sources: THREE.Object3D[] = [];
		scene.traverse((object) => {
			if (object.name.startsWith("geometry-source:")) sources.push(object);
		});
		expect(sources).toHaveLength(5);
		expect(
			sources.filter((source) => source.userData.layout === "matrix"),
		).toHaveLength(4);
		const emitter = scene.getObjectByName(
			`geometry-emitter:${mode.geometry.emitters[0].id}`,
		)!;
		expect(emitter.userData.sourceCount).toBe(4);
		expect(emitter.userData.beamAngleDegrees).toBeLessThan(
			emitter.userData.fieldAngleDegrees,
		);
		expect(emitter.userData.feather).toBe(0.35);
		expect(emitter.userData.focus).toBe(0.2);
		const cores: THREE.Object3D[] = [];
		scene.traverse((object) => {
			if (object.name === "beam-core") cores.push(object);
		});
		expect(cores).toHaveLength(5);
	});
});

describe("geometry emitter source layouts", () => {
	it("places point, ring, strip, matrix, and explicit-pixel beam sources", () => {
		const profile = blankFixtureProfile();
		profile.manufacturer = "Acme";
		profile.name = "Pixel Lamp";
		profile.revision = 1;
		const mode = profile.modes[0];
		mode.channels = [{ ...blankChannel(mode), attribute: "intensity" }];
		const nodeId = mode.geometry.nodes[0].id;
		const headId = mode.heads[0].id;
		const emitter = mode.geometry.emitters[0];
		mode.geometry.emitters = [
			{
				...emitter,
				id: "point",
				node_id: nodeId,
				head_id: headId,
				layout: { type: "point" },
			},
			{
				...emitter,
				id: "ring",
				node_id: nodeId,
				head_id: headId,
				layout: { type: "ring", count: 4, radius_millimetres: 100 },
			},
			{
				...emitter,
				id: "strip",
				node_id: nodeId,
				head_id: headId,
				layout: { type: "strip", count: 3, spacing_millimetres: 50 },
			},
			{
				...emitter,
				id: "matrix",
				node_id: nodeId,
				head_id: headId,
				layout: {
					type: "matrix",
					columns: 2,
					rows: 2,
					spacing: { x: 40, y: 30, z: 10 },
				},
			},
			{
				...emitter,
				id: "pixels",
				node_id: nodeId,
				head_id: headId,
				layout: {
					type: "explicit_pixels",
					positions: [
						{ x: 0, y: 0, z: 0 },
						{ x: 100, y: 200, z: 300 },
					],
				},
			},
		];
		const fixture = {
			fixture_id: profile.id,
			universe: 1,
			address: 1,
			definition: fixtureDefinitionFromProfileMode(profile, mode),
			logical_heads: [],
		} as PatchedFixture;
		const { scene } = buildStageScene(
			[
				{
					fixture,
					index: 0,
					position: {
						x: 0,
						y: 0,
						z: 3,
						rotationX: 0,
						rotationY: 0,
						rotationZ: 0,
					},
				},
			],
			null,
		);
		const sources: THREE.Object3D[] = [];
		scene.traverse((object) => {
			if (object.name.startsWith("geometry-source:")) sources.push(object);
		});

		expect(sources).toHaveLength(14);
		expect(
			Object.fromEntries(
				["point", "ring", "strip", "matrix", "explicit_pixels"].map(
					(layout) => [
						layout,
						sources.filter((source) => source.userData.layout === layout)
							.length,
					],
				),
			),
		).toEqual({ point: 1, ring: 4, strip: 3, matrix: 4, explicit_pixels: 2 });
		expect(
			scene.getObjectByName("geometry-source:pixels:1")?.position.toArray(),
		).toEqual([0.1, 0.2, 0.3]);
	});
});

describe("emitter direction and Patch selection", () => {
	it("uses the exact four render-quality representations for an active directional emitter", () => {
		const profile = blankFixtureProfile();
		const mode = profile.modes[0];
		mode.channels = [
			{ ...blankChannel(mode), attribute: "intensity", default_raw: 255 },
		];
		mode.geometry = geometryTemplate("fixed", [mode.heads[0].id]);
		const fixture = {
			fixture_id: profile.id,
			universe: 1,
			address: 1,
			definition: fixtureDefinitionFromProfileMode(profile, mode),
			logical_heads: [],
		} as PatchedFixture;
		const stageFixture = [
			{
				fixture,
				index: 0,
				position: {
					x: 0,
					y: 0,
					z: 3,
					rotationX: 0,
					rotationY: 0,
					rotationZ: 0,
				},
			},
		];
		const sceneFor = (
			quality: "lines_only" | "lines_and_beams" | "beams" | "improved_beams",
		) =>
			buildStageScene(
				stageFixture,
				null,
				new Set(),
				1,
				true,
				true,
				new Set(),
				quality,
			).scene;

		const lines = sceneFor("lines_only");
		expect(lines.getObjectByName("beam-centerline")?.visible).toBe(true);
		expect(lines.getObjectByName("beam-ground-footprint")?.visible).toBe(true);
		expect(lines.getObjectByName("beam-volume")?.visible).toBe(false);
		expect(lines.getObjectByName("beam-outline")).toBeUndefined();
		const combined = sceneFor("lines_and_beams");
		expect(combined.getObjectByName("beam-centerline")?.visible).toBe(true);
		expect(combined.getObjectByName("beam-ground-footprint")?.visible).toBe(
			true,
		);
		expect(combined.getObjectByName("beam-volume")?.visible).toBe(true);
		const beams = sceneFor("beams");
		expect(beams.getObjectByName("beam-centerline")?.visible).toBe(false);
		expect(beams.getObjectByName("beam-ground-footprint")?.visible).toBe(false);
		expect(beams.getObjectByName("beam-volume")?.visible).toBe(true);
		const improved = sceneFor("improved_beams");
		expect(improved.getObjectByName("beam-centerline")?.visible).toBe(false);
		expect(improved.getObjectByName("beam-improved-volume")?.visible).toBe(
			true,
		);
		expect(improved.getObjectByName("beam-volume")?.visible).toBe(false);
		expect(
			(improved.getObjectByName("beam-improved-volume") as THREE.Mesh).material,
		).toBeInstanceOf(THREE.ShaderMaterial);
		const retained = buildStageScene(
			stageFixture,
			null,
			new Set(),
			1,
			true,
			true,
			new Set(),
			"lines_only",
		);
		const retainedRoot = retained.fixtureObjects.values().next().value;
		expect(
			retained.scene.getObjectByName("beam-improved-volume"),
		).toBeUndefined();
		applyStageVisualization(
			stageFixture,
			null,
			retained.fixtureObjects,
			true,
			"improved_beams",
		);
		expect(retained.fixtureObjects.values().next().value).toBe(retainedRoot);
		const retainedImproved = retained.scene.getObjectByName(
			"beam-improved-volume",
		);
		expect(retainedImproved).toBeTruthy();
		expect(retainedImproved?.visible).toBe(true);
		expect(retained.scene.getObjectByName("beam-volume")?.visible).toBe(false);
		applyStageVisualization(
			stageFixture,
			null,
			retained.fixtureObjects,
			true,
			"beams",
		);
		expect(retained.fixtureObjects.values().next().value).toBe(retainedRoot);
		expect(
			retained.scene.getObjectByName("beam-improved-volume")?.visible,
		).toBe(false);
		expect(retained.scene.getObjectByName("beam-volume")?.visible).toBe(true);

		const withoutFloor = buildStageScene(
			stageFixture,
			null,
			new Set(),
			1,
			false,
			true,
			new Set(),
			"lines_only",
		).scene;
		expect(withoutFloor.getObjectByName("stage-floor")).toBeUndefined();
		expect(withoutFloor.getObjectByName("beam-ground-footprint")).toBeTruthy();
	});

	it("intersects the authored field cone with the ground exactly and hides non-finite footprints", () => {
		const sceneAt = (
			orientationX: number,
			defaultRaw = 255,
			showFloorGrid = true,
		) => {
			const profile = blankFixtureProfile();
			const mode = profile.modes[0];
			mode.channels = [
				{
					...blankChannel(mode),
					attribute: "intensity",
					default_raw: defaultRaw,
				},
			];
			mode.geometry = geometryTemplate("fixed", [mode.heads[0].id]);
			mode.geometry.emitters[0].orientation_degrees.x = orientationX;
			const fixture = {
				fixture_id: profile.id,
				universe: 1,
				address: 1,
				definition: fixtureDefinitionFromProfileMode(profile, mode),
				logical_heads: [],
			} as PatchedFixture;
			return buildStageScene(
				[
					{
						fixture,
						index: 0,
						position: {
							x: 0,
							y: 0,
							z: 3,
							rotationX: 0,
							rotationY: 0,
							rotationZ: 0,
						},
					},
				],
				null,
				new Set(),
				1,
				showFloorGrid,
				true,
				new Set(),
				"lines_only",
			).scene;
		};
		const footprintPoints = (scene: THREE.Scene) => {
			const footprint = scene.getObjectByName(
				"beam-ground-footprint",
			) as THREE.LineLoop;
			const positions = footprint.geometry.getAttribute(
				"position",
			) as THREE.BufferAttribute;
			return {
				footprint,
				points: Array.from({ length: positions.count }, (_, index) =>
					new THREE.Vector3().fromBufferAttribute(positions, index),
				),
			};
		};

		const perpendicular = sceneAt(0);
		const perpendicularFootprint = footprintPoints(perpendicular);
		expect(perpendicularFootprint.footprint.visible).toBe(true);
		const perpendicularX = perpendicularFootprint.points.map(({ x }) => x);
		const perpendicularZ = perpendicularFootprint.points.map(({ z }) => z);
		expect(
			Math.max(...perpendicularX) - Math.min(...perpendicularX),
		).toBeCloseTo(Math.max(...perpendicularZ) - Math.min(...perpendicularZ), 4);

		const oblique = sceneAt(30);
		const obliqueFootprint = footprintPoints(oblique);
		expect(obliqueFootprint.footprint.visible).toBe(true);
		let source: THREE.Object3D | undefined;
		oblique.traverse((object) => {
			if (!source && object.name.startsWith("geometry-source:"))
				source = object;
		});
		expect(source).toBeTruthy();
		const origin =
			source?.getWorldPosition(new THREE.Vector3()) ?? new THREE.Vector3();
		const axis = new THREE.Vector3(0, -1, 0)
			.applyQuaternion(
				source?.getWorldQuaternion(new THREE.Quaternion()) ??
					new THREE.Quaternion(),
			)
			.normalize();
		const expectedSlope =
			Number(source?.userData.stageBeamRadius) /
			Number(source?.userData.stageBeamDistance);
		for (const point of obliqueFootprint.points.slice(0, -1)) {
			const offset = point.clone().sub(origin);
			const axial = offset.dot(axis);
			const radial = offset.clone().addScaledVector(axis, -axial).length();
			expect(radial / axial).toBeCloseTo(expectedSlope, 4);
		}
		const obliqueX = obliqueFootprint.points.map(({ x }) => x);
		const obliqueZ = obliqueFootprint.points.map(({ z }) => z);
		expect(Math.max(...obliqueZ) - Math.min(...obliqueZ)).toBeGreaterThan(
			Math.max(...obliqueX) - Math.min(...obliqueX),
		);

		expect(footprintPoints(sceneAt(90)).footprint.visible).toBe(false);
		expect(footprintPoints(sceneAt(180)).footprint.visible).toBe(false);
		const withoutFloor = sceneAt(0, 255, false);
		expect(withoutFloor.getObjectByName("stage-floor")).toBeUndefined();
		expect(footprintPoints(withoutFloor).footprint.visible).toBe(true);

		const lowScene = sceneAt(0, 16);
		const highScene = sceneAt(0, 255);
		const lowMaterial = footprintPoints(lowScene).footprint
			.material as THREE.LineBasicMaterial;
		const highMaterial = footprintPoints(highScene).footprint
			.material as THREE.LineBasicMaterial;
		expect(lowMaterial.opacity).toBeGreaterThanOrEqual(0.18);
		expect(highMaterial.opacity).toBeGreaterThan(lowMaterial.opacity);
		expect(highMaterial.opacity).toBeLessThanOrEqual(0.68);
		const lowCenter = lowScene.getObjectByName("beam-centerline") as THREE.Line;
		const highCenter = highScene.getObjectByName(
			"beam-centerline",
		) as THREE.Line;
		expect(
			(highCenter.material as THREE.LineBasicMaterial).opacity,
		).toBeGreaterThan((lowCenter.material as THREE.LineBasicMaterial).opacity);
	});

	it("uses the bounded Improved-beam shader for conservative fallback fixtures", () => {
		const fallback = fixture("moving wash", "Fallback moving wash");
		const stageFixture = [
			{
				fixture: fallback,
				index: 0,
				position: {
					x: 0,
					y: 0,
					z: 3,
					rotationX: 0,
					rotationY: 0,
					rotationZ: 0,
				},
			},
		];
		const snapshot: VisualizationSnapshot = {
			revision: 1,
			generated_at: "",
			grand_master: 1,
			blackout: false,
			values: [
				{
					fixture_id: fallback.fixture_id,
					attribute: "intensity",
					value: { kind: "normalized", value: 1 },
				},
			],
		};
		const built = buildStageScene(
			stageFixture,
			snapshot,
			new Set(),
			1,
			true,
			true,
			new Set(),
			"improved_beams",
		);
		const improved = built.scene.getObjectByName(
			"beam-improved-volume",
		) as THREE.Mesh;
		expect(improved.visible).toBe(true);
		expect(improved.material).toBeInstanceOf(THREE.ShaderMaterial);
		expect(built.scene.getObjectByName("beam-volume")?.visible).toBe(false);
		expect(built.scene.getObjectByName("beam-outline")).toBeUndefined();
	});

	it("uses emitter direction metadata and keeps an inactive geometry source readable", () => {
		const profile = blankFixtureProfile();
		const mode = profile.modes[0];
		mode.channels = [
			{ ...blankChannel(mode), attribute: "intensity", default_raw: 0 },
		];
		mode.geometry = geometryTemplate("fixed", [mode.heads[0].id]);
		const fixture = {
			fixture_id: profile.id,
			universe: 1,
			address: 1,
			definition: fixtureDefinitionFromProfileMode(profile, mode),
			logical_heads: [],
		} as PatchedFixture;
		const stageFixture = [
			{
				fixture,
				index: 0,
				position: {
					x: 0,
					y: 0,
					z: 3,
					rotationX: 0,
					rotationY: 0,
					rotationZ: 0,
				},
			},
		];
		const directional = buildStageScene(stageFixture, null);
		expect(
			directional.scene.getObjectByName("beam-direction-guide"),
		).toBeTruthy();
		const source = directional.scene.getObjectByName(
			"light-emitting-surface",
		) as THREE.Mesh;
		expect(source.userData.active).toBe(false);
		expect(source.material).toBeInstanceOf(THREE.MeshStandardMaterial);

		mode.geometry.emitters[0].directional = false;
		fixture.definition = fixtureDefinitionFromProfileMode(profile, mode);
		const broad = buildStageScene(stageFixture, null);
		expect(broad.scene.getObjectByName("beam-direction-guide")).toBeUndefined();
		expect(broad.scene.getObjectByName("light-emitting-surface")).toBeTruthy();
	});

	it("illuminates a Patch-selected fixture virtually without changing its live snapshot", () => {
		const profile = blankFixtureProfile();
		const mode = profile.modes[0];
		mode.channels = [
			{ ...blankChannel(mode), attribute: "intensity", default_raw: 0 },
		];
		mode.geometry = geometryTemplate("fixed", [mode.heads[0].id]);
		const fixture = {
			fixture_id: profile.id,
			universe: 1,
			address: 1,
			definition: fixtureDefinitionFromProfileMode(profile, mode),
			logical_heads: [],
		} as PatchedFixture;
		const stageFixture = [
			{
				fixture,
				index: 0,
				position: {
					x: 0,
					y: 0,
					z: 3,
					rotationX: 0,
					rotationY: 0,
					rotationZ: 0,
				},
			},
		];
		const snapshot: VisualizationSnapshot = {
			revision: 1,
			generated_at: "",
			grand_master: 0,
			blackout: true,
			values: [],
		};

		const live = buildStageScene(stageFixture, snapshot);
		expect(
			live.scene.getObjectByName("light-emitting-surface")?.userData.active,
		).toBe(false);
		const preview = buildStageScene(
			stageFixture,
			snapshot,
			new Set(),
			1,
			true,
			true,
			new Set([profile.id]),
		);
		expect(
			preview.scene.getObjectByName("light-emitting-surface")?.userData.active,
		).toBe(true);
		expect(snapshot).toEqual({
			revision: 1,
			generated_at: "",
			grand_master: 0,
			blackout: true,
			values: [],
		});
	});
});

describe("fixture profile model mounting", () => {
	it("mounts named GLB parts on their profile geometry anchors", () => {
		const profile = blankFixtureProfile();
		profile.manufacturer = "Acme";
		profile.name = "Bound Mover";
		profile.revision = 1;
		const mode = profile.modes[0];
		mode.geometry = geometryTemplate("moving_head", [mode.heads[0].id]);
		const pan = mode.geometry.nodes.find(
			(node) => node.motion?.attribute === "pan",
		)!;
		const tilt = mode.geometry.nodes.find(
			(node) => node.motion?.attribute === "tilt",
		)!;
		pan.glb_node = "PanVisual";
		tilt.glb_node = "TiltVisual";
		const fixture = {
			fixture_id: profile.id,
			universe: 1,
			address: 1,
			definition: fixtureDefinitionFromProfileMode(profile, mode),
			logical_heads: [],
		} as PatchedFixture;
		const { scene, fixtureObjects } = buildStageScene(
			[
				{
					fixture,
					index: 0,
					position: {
						x: 0,
						y: 0,
						z: 3,
						rotationX: 0,
						rotationY: 0,
						rotationZ: 0,
					},
				},
			],
			{
				revision: 1,
				generated_at: "",
				grand_master: 1,
				blackout: false,
				values: [
					{
						fixture_id: profile.id,
						attribute: "pan",
						value: { kind: "normalized", value: 0.75 },
					},
				],
			},
		);
		const model = new THREE.Group();
		const panVisual = new THREE.Mesh(new THREE.BoxGeometry(1, 1, 1));
		panVisual.name = "PanVisual";
		const tiltVisual = new THREE.Mesh(new THREE.SphereGeometry(0.5));
		tiltVisual.name = "TiltVisual";
		panVisual.add(tiltVisual);
		model.add(panVisual);
		const root = fixtureObjects.get(profile.id)!;

		expect(mountFixtureModel(root, model, fixture)).toBe(2);
		const panPart = scene.getObjectByName(`fixture-model-part:${pan.id}`)!;
		const tiltPart = scene.getObjectByName(`fixture-model-part:${tilt.id}`)!;
		expect(panPart.parent?.name).toBe(`geometry-node-anchor:${pan.id}`);
		expect(tiltPart.parent?.name).toBe(`geometry-node-anchor:${tilt.id}`);
		expect(panPart.getObjectByName("PanVisual")).toBeTruthy();
		expect(panPart.getObjectByName("TiltVisual")).toBeUndefined();
		expect(tiltPart.getObjectByName("TiltVisual")).toBeTruthy();
		expect(scene.getObjectByName(`geometry-part:${pan.id}`)).toBeUndefined();
		expect(
			scene.getObjectByName(`geometry-node:${pan.id}`)?.rotation.y,
		).toBeCloseTo(THREE.MathUtils.degToRad(135));
	});

	it("mounts metre-authored visual-only geometry without emitters or normalization", () => {
		const profile = blankFixtureProfile();
		profile.manufacturer = "Venue";
		profile.name = "Two-Point Truss";
		profile.revision = 1;
		profile.patch_policy = "visual_only";
		profile.model_units = "metres";
		const mode = profile.modes[0];
		mode.splits[0].footprint = 0;
		mode.geometry.emitters = [];
		mode.geometry.nodes[0].glb_node = "Truss2m";
		const fixture = {
			fixture_id: profile.id,
			universe: null,
			address: null,
			definition: fixtureDefinitionFromProfileMode(profile, mode),
			logical_heads: [],
		} as PatchedFixture;
		const { scene, fixtureObjects } = buildStageScene(
			[
				{
					fixture,
					index: 0,
					position: {
						x: 0,
						y: 0,
						z: 0,
						rotationX: 0,
						rotationY: 0,
						rotationZ: 0,
					},
				},
			],
			null,
		);
		expect(
			scene.getObjectByName(`geometry-node:${mode.geometry.nodes[0].id}`),
		).toBeTruthy();
		const model = new THREE.Group();
		const truss = new THREE.Mesh(new THREE.BoxGeometry(2, 0.3, 0.3));
		truss.name = "Truss2m";
		model.add(truss);
		const root = fixtureObjects.get(profile.id)!;
		expect(mountFixtureModel(root, model, fixture)).toBe(1);
		const mounted = scene.getObjectByName(
			`fixture-model-part:${mode.geometry.nodes[0].id}`,
		)!;
		expect(mounted.scale.toArray()).toEqual([1, 1, 1]);
		expect(
			new THREE.Box3().setFromObject(mounted).getSize(new THREE.Vector3()).x,
		).toBeCloseTo(2);
	});

	it("uses conservative fallback beams for lighting profiles without emitters", () => {
		const profile = blankFixtureProfile();
		profile.name = "Legacy moving light";
		profile.revision = 1;
		const mode = profile.modes[0];
		mode.geometry.emitters = [];
		const fixture = {
			fixture_id: profile.id,
			universe: 1,
			address: 1,
			definition: fixtureDefinitionFromProfileMode(profile, mode),
			logical_heads: [],
		} as PatchedFixture;
		const { scene } = buildStageScene(
			[
				{
					fixture,
					index: 0,
					position: {
						x: 0,
						y: 0,
						z: 2,
						rotationX: 0,
						rotationY: 0,
						rotationZ: 0,
					},
				},
			],
			{
				revision: 1,
				generated_at: "2026-07-28T10:00:00Z",
				grand_master: 1,
				blackout: false,
				preload: false,
				values: [
					{
						fixture_id: profile.id,
						attribute: "intensity",
						value: { kind: "normalized", value: 0.2 },
					},
				],
				profile_output_values: [],
			},
		);

		expect(scene.getObjectByName("fallback-beam")).toBeTruthy();
		expect(scene.getObjectByName("beam-volume")?.visible).toBe(true);
		const center = scene.getObjectByName("beam-centerline") as THREE.Line;
		expect(center.visible).toBe(true);
		expect((center.material as THREE.LineBasicMaterial).opacity).toBeCloseTo(
			0.53,
		);
	});
});

describe("calibrated fixture output", () => {
	it("uses post-profile calibrated color and mastered intensity without applying desk masters twice", () => {
		const profile = blankFixtureProfile();
		profile.manufacturer = "Acme";
		profile.name = "Projected Lamp";
		profile.revision = 1;
		const mode = profile.modes[0];
		mode.channels = [{ ...blankChannel(mode), attribute: "intensity" }];
		const fixture = {
			fixture_id: profile.id,
			universe: 1,
			address: 1,
			definition: fixtureDefinitionFromProfileMode(profile, mode),
			logical_heads: [],
		} as PatchedFixture;
		const { scene } = buildStageScene(
			[
				{
					fixture,
					index: 0,
					position: {
						x: 0,
						y: 0,
						z: 3,
						rotationX: 0,
						rotationY: 0,
						rotationZ: 0,
					},
				},
			],
			{
				revision: 1,
				generated_at: "",
				grand_master: 0.1,
				blackout: true,
				values: [
					{
						fixture_id: profile.id,
						attribute: "intensity",
						value: { kind: "normalized", value: 1 },
					},
					{
						fixture_id: profile.id,
						attribute: "color",
						value: {
							kind: "color_xyz",
							value: { x: 0.4124564, y: 0.2126729, z: 0.0193339 },
						},
					},
				],
				profile_output_values: [
					{
						fixture_id: profile.id,
						attribute: "intensity",
						value: { kind: "normalized", value: 0.25 },
					},
					{
						fixture_id: profile.id,
						attribute: "color",
						value: {
							kind: "color_xyz",
							value: { x: 0.1804375, y: 0.072175, z: 0.9503041 },
						},
					},
				],
			},
		);
		const emitter = scene.getObjectByName(
			`geometry-emitter:${mode.geometry.emitters[0].id}`,
		)!;
		expect(emitter.userData.intensity).toBe(0.25);
		expect(emitter.userData.color).toBe("#0000ff");
	});
});

describe("built-in fixture family mapping and motion", () => {
	it("recognizes the requested fixture families", () => {
		expect(inferBuiltInFixtureKind(fixture("moving wash", "A7 LED Wash"))).toBe(
			"wash-led",
		);
		expect(inferBuiltInFixtureKind(fixture("moving profile", "Profile"))).toBe(
			"profile",
		);
		expect(
			inferBuiltInFixtureKind(fixture("dimmer profile", "Dimmer Profile")),
		).toBe("profile-static");
		expect(inferBuiltInFixtureKind(fixture("wash", "Classic Wash"))).toBe(
			"wash-classic",
		);
		expect(inferBuiltInFixtureKind(fixture("conventional", "PAR Can"))).toBe(
			"par",
		);
		expect(inferBuiltInFixtureKind(fixture("conventional", "PC Fresnel"))).toBe(
			"fresnel",
		);
		expect(inferBuiltInFixtureKind(fixture("strobe", "Strobe"))).toBe("strobe");
		expect(inferBuiltInFixtureKind(fixture("strip light", "Sunstrip"))).toBe(
			"sunstrip",
		);
	});

	it("maps tilt symmetrically from minus 160 to plus 160 degrees", () => {
		expect(THREE.MathUtils.radToDeg(movingLightTiltRadians(0))).toBeCloseTo(
			-160,
		);
		expect(THREE.MathUtils.radToDeg(movingLightTiltRadians(0.5))).toBeCloseTo(
			0,
		);
		expect(THREE.MathUtils.radToDeg(movingLightTiltRadians(1))).toBeCloseTo(
			160,
		);
	});

	it("tilts a moving head on the axle between the yoke arms", () => {
		const model = createBuiltInFixtureModel(
			fixture("moving profile", "Profile"),
			new THREE.Color("white"),
			1,
			0,
			movingLightTiltRadians(0.75),
		);
		const tiltGroup = model.beamMount.parent!;
		expect(tiltGroup.rotation.x).toBeCloseTo(THREE.MathUtils.degToRad(80));
		expect(tiltGroup.rotation.z).toBeCloseTo(0);
	});

	it("keeps a square base fixed while the centered yoke pans and the head tilts", () => {
		const neutral = createBuiltInFixtureModel(
			fixture("moving wash", "A7 LED Wash"),
			new THREE.Color("white"),
			1,
			0,
			0,
		);
		const moved = createBuiltInFixtureModel(
			fixture("moving wash", "A7 LED Wash"),
			new THREE.Color("white"),
			1,
			0.8,
			movingLightTiltRadians(0.75),
		);
		const base = moved.object.getObjectByName(
			"fixed-square-base",
		) as THREE.Mesh;
		const yoke = moved.object.getObjectByName("centered-rotating-yoke")!;
		const head = moved.object.getObjectByName("moving-head-body")!;
		const lens = moved.object.getObjectByName("light-emitting-surface")!;
		expect(lens.userData.fixturePart).toBe("moving-front-lens");
		const size = new THREE.Box3()
			.setFromObject(base)
			.getSize(new THREE.Vector3());
		expect(base.geometry).toBeInstanceOf(THREE.BoxGeometry);
		expect(size.x).toBeCloseTo(size.z);
		expect(base.parent).toBe(moved.object);
		expect(yoke.parent).toBe(moved.object);
		expect(base.rotation.y).toBeCloseTo(
			neutral.object.getObjectByName("fixed-square-base")!.rotation.y,
		);
		expect(yoke.rotation.y).toBeCloseTo(0.8);
		expect(head.parent?.name).toBe("tilting-head");
		expect(lens.parent?.name).toBe("tilting-head");
		expect(moved.beamMount.parent?.name).toBe("tilting-head");
		expect(moved.beamMount.position).toEqual(lens.position);
	});
});

describe("built-in emitting surfaces and scene construction", () => {
	it("gives every fixture family a bright unlit emitting surface", () => {
		for (const [type, name] of [
			["moving wash", "A7 LED Wash"],
			["moving profile", "Profile"],
			["dimmer profile", "Dimmer Profile"],
			["wash", "Classic Wash"],
			["scanner", "Mirror Mover"],
			["conventional", "PAR Can"],
			["conventional", "PC Fresnel"],
			["strobe", "Strobe"],
			["strip light", "Sunstrip"],
		]) {
			const model = createBuiltInFixtureModel(
				fixture(type, name),
				new THREE.Color(0x55aaff),
				1,
				0,
				0,
			);
			const sources: THREE.Mesh[] = [];
			model.object.traverse((object) => {
				if (
					object instanceof THREE.Mesh &&
					object.name.startsWith("light-emitting-surface")
				)
					sources.push(object);
			});
			expect(sources.length, name).toBeGreaterThan(0);
			expect(
				sources.every(
					(source) => source.material instanceof THREE.MeshBasicMaterial,
				),
				name,
			).toBe(true);
		}
	});

	it("builds a selected Sunstrip scene without invalid outline geometry", () => {
		const sunstrip = fixture("strip light", "Sunstrip");
		expect(() =>
			buildStageScene(
				[
					{
						fixture: sunstrip,
						index: 0,
						position: {
							x: 0,
							y: 0,
							z: 3,
							rotationX: 0,
							rotationY: 0,
							rotationZ: 0,
						},
					},
				],
				null,
				new Set([sunstrip.fixture_id]),
			),
		).not.toThrow();
	});
});

describe("built-in direction guides and external models", () => {
	it("shows off-state direction guides for fixed and moving directional lamps only", () => {
		const fresnel = fixture("dimmer fresnel", "Dimmer Fresnel");
		const mover = fixture("moving wash", "A7 LED Wash");
		const blinder = fixture("blinder", "Audience Blinder");
		const strobe = fixture("strobe", "Strobe");
		const sunstrip = fixture("strip light", "Sunstrip");
		expect([fresnel, mover, blinder].every(fallbackEmitterIsDirectional)).toBe(
			true,
		);
		expect([strobe, sunstrip].some(fallbackEmitterIsDirectional)).toBe(false);
		const stageFixture = (item: PatchedFixture, index: number) => ({
			fixture: item,
			index,
			position: {
				x: index,
				y: 0,
				z: 3,
				rotationX: 0,
				rotationY: 0,
				rotationZ: 0,
			},
		});
		const withGuides = buildStageScene(
			[fresnel, mover, blinder, strobe, sunstrip].map(stageFixture),
			null,
		);
		const guides: THREE.Object3D[] = [];
		withGuides.scene.traverse((object) => {
			if (object.name === "beam-direction-guide") guides.push(object);
		});
		expect(guides).toHaveLength(3);
		const hidden = buildStageScene(
			[fresnel, mover, blinder].map(stageFixture),
			null,
			new Set(),
			1,
			true,
			false,
		);
		const hiddenGuides: THREE.Object3D[] = [];
		hidden.scene.traverse((object) => {
			if (object.name === "beam-direction-guide") hiddenGuides.push(object);
		});
		expect(hiddenGuides).toHaveLength(3);
		expect(hiddenGuides.every((guide) => !guide.visible)).toBe(true);
	});

	it("keeps an inactive emitter surface visible when an external fixture model is mounted", () => {
		const wash = fixture("moving wash", "Modelled Wash");
		wash.definition.model_asset =
			"data:model/gltf-binary;base64,unused-by-scene-builder";
		const { scene } = buildStageScene(
			[
				{
					fixture: wash,
					index: 0,
					position: {
						x: 0,
						y: 0,
						z: 3,
						rotationX: 0,
						rotationY: 0,
						rotationZ: 0,
					},
				},
			],
			null,
		);
		const source = scene.getObjectByName(
			"light-emitting-surface",
		) as THREE.Mesh;
		expect(source).toBeTruthy();
		expect(source.userData.active).toBe(false);
		expect(source.material).toBeInstanceOf(THREE.MeshStandardMaterial);
	});
});

describe("built-in wash and conventional housings", () => {
	it("uses one filled central source for a wash mover instead of an LED ring", () => {
		const beamColor = new THREE.Color(0xff0000);
		const model = createBuiltInFixtureModel(
			fixture("moving wash", "A7 LED Wash"),
			beamColor,
			1,
			0,
			0,
		);
		const sources: THREE.Mesh[] = [];
		model.object.traverse((object) => {
			if (
				object instanceof THREE.Mesh &&
				object.name === "light-emitting-surface"
			)
				sources.push(object);
		});
		expect(sources).toHaveLength(1);
		expect(sources[0].geometry).toBeInstanceOf(THREE.CircleGeometry);
		const sourceColor = (sources[0].material as THREE.MeshBasicMaterial).color;
		expect(sourceColor.r).toBeGreaterThanOrEqual(sourceColor.g);
		expect(sourceColor.g).toBeGreaterThan(0.7);
		expect(sourceColor.b).toBeGreaterThan(0.7);
	});

	it("gives the conventional dimmers their recognizable practical housings", () => {
		const par = createBuiltInFixtureModel(
			fixture("dimmer par can", "Dimmer PAR Can"),
			new THREE.Color("white"),
			1,
			0,
			0,
		);
		const gelFrame = par.object.getObjectByName("par-gel-frame")!;
		expect(gelFrame.children).toHaveLength(4);
		const frameSize = new THREE.Box3()
			.setFromObject(gelFrame)
			.getSize(new THREE.Vector3());
		expect(frameSize.y).toBeCloseTo(frameSize.z);

		const profile = createBuiltInFixtureModel(
			fixture("dimmer profile", "Dimmer Profile"),
			new THREE.Color("white"),
			1,
			0,
			0,
		);
		expect(profile.object.getObjectByName("profile-shutter-gate")).toBeTruthy();
		expect(profile.object.getObjectByName("profile-lens-barrel")).toBeTruthy();
		const profileSize = new THREE.Box3()
			.setFromObject(profile.object)
			.getSize(new THREE.Vector3());
		expect(profileSize.y / profileSize.z).toBeGreaterThan(1.5);

		const fresnel = createBuiltInFixtureModel(
			fixture("dimmer fresnel", "Dimmer Fresnel"),
			new THREE.Color("white"),
			1,
			0,
			0,
		);
		const doors: THREE.Object3D[] = [];
		fresnel.object.traverse((object) => {
			if (object.name.startsWith("fresnel-barn-door-")) doors.push(object);
		});
		expect(doors.map((door) => door.name).sort()).toEqual([
			"fresnel-barn-door-bottom",
			"fresnel-barn-door-left",
			"fresnel-barn-door-right",
			"fresnel-barn-door-top",
		]);
	});
});

describe("built-in off-state lenses and scanner motion", () => {
	it("renders an off lens as visible neutral glass without making it look lit", () => {
		const model = createBuiltInFixtureModel(
			fixture("moving profile", "Profile"),
			new THREE.Color(0xff0000),
			0,
			0,
			0,
		);
		let source: THREE.Mesh | undefined;
		model.object.traverse((object) => {
			if (
				object instanceof THREE.Mesh &&
				object.name === "light-emitting-surface"
			)
				source = object;
		});
		const color = (source!.material as THREE.MeshBasicMaterial).color;
		expect(Math.max(color.r, color.g, color.b)).toBeGreaterThan(0.05);
		expect(Math.max(color.r, color.g, color.b)).toBeLessThan(0.2);
		expect(
			Math.max(color.r, color.g, color.b) - Math.min(color.r, color.g, color.b),
		).toBeLessThan(0.04);
	});

	it("builds a scanner with a fixed source and animated 45-degree mirror", () => {
		const scanner = fixture("scanner", "Mirror Mover Scanner");
		expect(inferBuiltInFixtureKind(scanner)).toBe("mirror-scanner");
		const neutral = createBuiltInFixtureModel(
			scanner,
			new THREE.Color("white"),
			1,
			0,
			0,
		);
		const mirror = neutral.object.getObjectByName("moving-mirror")!;
		const chassis = neutral.object.getObjectByName(
			"scanner-chassis",
		) as THREE.Mesh;
		const chassisSize = new THREE.Box3()
			.setFromObject(chassis)
			.getSize(new THREE.Vector3());
		expect(chassisSize.z / chassisSize.x).toBeCloseTo(3);
		expect(mirror.parent!.rotation.x).toBeCloseTo(Math.PI / 4);
		const moved = createBuiltInFixtureModel(
			scanner,
			new THREE.Color("white"),
			1,
			0.4,
			movingLightTiltRadians(0.75),
		);
		const movedMirror = moved.object.getObjectByName("moving-mirror")!;
		expect(movedMirror.parent!.rotation.x).not.toBeCloseTo(Math.PI / 4);
		expect(moved.beamMount.parent!.rotation.y).toBeCloseTo(0.4);
	});
});
