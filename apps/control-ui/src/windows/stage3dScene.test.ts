import { describe, expect, it } from "vitest";
import type { VisualizationSnapshot } from "../api/types";
import {
	buildStageScene,
	cueVisualization,
	fallbackEmitterIsDirectional,
	migrateStagePosition,
	mountFixtureModel,
} from "./stage3dScene";
import {
	createBuiltInFixtureModel,
	inferBuiltInFixtureKind,
	movingLightTiltRadians,
} from "./builtInStageModels";
import * as THREE from "three";
import type { PatchedFixture } from "../api/types";
import {
	blankChannel,
	blankFixtureProfile,
	blankHead,
	fixtureDefinitionFromProfileMode,
	geometryTemplate,
} from "../components/setup/fixtureProfileModel";

describe("3D stage state", () => {
	it("can omit the floor plane and grid from the scene", () => {
		const visible = buildStageScene([], null);
		expect(visible.scene.getObjectByName("stage-floor")).toBeTruthy();
		expect(visible.scene.getObjectByName("stage-floor-grid")).toBeTruthy();

		const hidden = buildStageScene([], null, new Set(), 1, false);
		expect(hidden.scene.getObjectByName("stage-floor")).toBeUndefined();
		expect(hidden.scene.getObjectByName("stage-floor-grid")).toBeUndefined();
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

describe("built-in 3D model library", () => {
	const fixture = (device_type: string, name: string) =>
		({
			fixture_id: "fixture",
			universe: 1,
			address: 1,
			definition: { device_type, name, manufacturer: "", model: name },
		}) as PatchedFixture;

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
		expect(
			hidden.scene.getObjectByName("beam-direction-guide"),
		).toBeUndefined();
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
		expect(profileSize.x / profileSize.z).toBeGreaterThan(1.5);

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
