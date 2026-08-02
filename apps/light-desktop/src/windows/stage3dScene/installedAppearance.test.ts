import * as THREE from "three";
import { describe, expect, it } from "vitest";
import type { PatchedFixture, VisualizationSnapshot } from "../../api/types";
import { buildStageScene, disposeScene } from "../stage3dScene";
import {
	applyInstalledAppearance,
	colorTemperatureLinearRgb,
	parseCanonicalSrgbHexLinear,
} from "./installedAppearance";

const fixture = {
	fixture_id: "fixture",
	universe: 1,
	address: 1,
	logical_heads: [],
	definition: {
		device_type: "profile",
		manufacturer: "",
		name: "Source",
		model: "Source",
	},
} as unknown as PatchedFixture;

const position = {
	x: 0,
	y: 0,
	z: 3,
	rotationX: 0,
	rotationY: 0,
	rotationZ: 0,
};

describe("installed Stage appearance", () => {
	it("matches the standalone renderer's stable linear CCT and gel composition", () => {
		const warm = colorTemperatureLinearRgb(3_200);
		expect(warm.r).toBeCloseTo(1, 5);
		expect(warm.g).toBeCloseTo(0.477115, 5);
		expect(warm.b).toBeCloseTo(0.198484, 5);
		const cool = colorTemperatureLinearRgb(10_000);
		expect(cool.r).toBeCloseTo(0.588681, 5);
		expect(cool.g).toBeCloseTo(0.701615, 5);
		expect(cool.b).toBeCloseTo(1, 5);
		const result = applyInstalledAppearance(
			new THREE.Color(0.8, 0.6, 0.4),
			fixture,
			{
				light_source: { type: "tungsten" },
				color_temperature_kelvin: 3_200,
				gel: { type: "custom", name: "Red", color_srgb: "#C01020", note: null },
				shaper_angles_degrees: [0, 0, 0, 0],
			},
		);
		expect(result.r).toBeCloseTo(0.421692, 5);
		expect(result.g).toBeCloseTo(0.001483, 5);
		expect(result.b).toBeCloseTo(0.001147, 5);
		expect(parseCanonicalSrgbHexLinear("#c01020")).toBeNull();
	});

	it("colors root and copy emissive surfaces, beams, footprints, and spill independently", () => {
		const snapshot: VisualizationSnapshot = {
			revision: 1,
			generated_at: "",
			grand_master: 1,
			blackout: false,
			values: [
				{
					fixture_id: fixture.fixture_id,
					attribute: "intensity",
					value: { kind: "normalized", value: 1 },
				},
			],
		};
		const open = {
			light_source: { type: "profile_default" as const },
			color_temperature_kelvin: null,
			gel: { type: "open_white" as const },
			shaper_angles_degrees: [0, 0, 0, 0] as [number, number, number, number],
		};
		const red = {
			...open,
			gel: {
				type: "built_in" as const,
				catalog_id: "missing-catalog",
				entry_id: "missing-entry",
				embedded_fallback: {
					number: "R",
					name: "Red",
					display_srgb: "#FF0000",
					visualizer_srgb: "#FF0000",
				},
			},
		};
		const built = buildStageScene(
			[
				{
					fixture,
					instanceId: "root",
					installedAppearance: open,
					index: 0,
					position,
				},
				{
					fixture,
					instanceId: "copy",
					installedAppearance: red,
					index: 1,
					position: { ...position, x: 1 },
				},
			],
			snapshot,
			new Set(),
			1,
			true,
			true,
			new Set(),
			"improved_beams",
		);
		const rootBeam = built.fixtureObjects
			.get("root")
			?.getObjectByName("fallback-beam") as THREE.Object3D;
		const copyBeam = built.fixtureObjects
			.get("copy")
			?.getObjectByName("fallback-beam") as THREE.Object3D;
		expect(rootBeam.userData.stageBeamColor).toBe("#ffffff");
		expect(copyBeam.userData.stageBeamColor).toBe("#ff0000");
		const copySurface = built.fixtureObjects
			.get("copy")
			?.getObjectByName("light-emitting-surface") as THREE.Mesh;
		const copyVolume = copyBeam.getObjectByName(
			"beam-improved-volume",
		) as THREE.Mesh;
		const surfaceColor = (copySurface.material as THREE.MeshBasicMaterial)
			.color;
		expect(surfaceColor.r).toBeGreaterThan(surfaceColor.g);
		expect(
			(copyVolume.material as THREE.ShaderMaterial).uniforms.beamColor.value.g,
		).toBe(0);
		const spill = copyBeam.getObjectByName(
			"stage-improved-spotlight",
		) as THREE.SpotLight;
		expect(spill.color.g).toBe(0);
		const footprints = built.scene.userData.stageGroundFootprints as Map<
			string,
			THREE.LineLoop
		>;
		expect(
			(footprints.get(copyBeam.uuid)?.material as THREE.LineBasicMaterial).color
				.g,
		).toBe(0);
		disposeScene(built.scene);
	});
});
