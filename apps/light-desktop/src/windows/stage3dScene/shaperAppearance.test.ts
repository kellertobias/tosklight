import { describe, expect, it } from "vitest";
import type { PatchedFixture, VisualizationSnapshot } from "../../api/types";
import {
	blankChannel,
	blankFixtureProfile,
	fixtureDefinitionFromProfileMode,
	geometryTemplate,
} from "../../components/setup/fixtureProfileModel";
import { buildStageScene, disposeScene } from "../stage3dScene";

describe("typed Stage shaper roles", () => {
	it("uses static angles only for supported roles and lets live physical values take ownership", () => {
		const profile = blankFixtureProfile();
		const mode = profile.modes[0];
		mode.geometry = geometryTemplate("fixed", [mode.heads[0].id]);
		mode.channels = [
			{ ...blankChannel(mode), attribute: "intensity" },
			{ ...blankChannel(mode), attribute: "shaper.blade.1.position" },
			{
				...blankChannel(mode),
				attribute: "shaper.blade.2.angle",
				physical_min: -90,
				physical_max: 90,
				unit: "degrees",
			},
			{
				...blankChannel(mode),
				attribute: "shaper.rotation",
				physical_min: -180,
				physical_max: 180,
				unit: "degrees",
			},
		];
		const fixture = {
			fixture_id: profile.id,
			universe: 1,
			address: 1,
			logical_heads: [],
			definition: fixtureDefinitionFromProfileMode(profile, mode),
		} as PatchedFixture;
		const snapshot: VisualizationSnapshot = {
			revision: 1,
			generated_at: "",
			grand_master: 1,
			blackout: false,
			values: [
				{
					fixture_id: profile.id,
					attribute: "intensity",
					value: { kind: "normalized", value: 1 },
				},
				{
					fixture_id: profile.id,
					attribute: "shaper.blade.1.position",
					value: { kind: "normalized", value: 0.5 },
				},
				{
					fixture_id: profile.id,
					attribute: "shaper.blade.2.angle",
					value: { kind: "normalized", value: 0.25 },
				},
				{
					fixture_id: profile.id,
					attribute: "shaper.rotation",
					value: { kind: "normalized", value: 0.25 },
				},
			],
		};
		const built = buildStageScene(
			[
				{
					fixture,
					installedAppearance: {
						light_source: { type: "profile_default" },
						color_temperature_kelvin: null,
						gel: { type: "open_white" },
						shaper_angles_degrees: [10, 20, 30, 40],
					},
					shaperAngle: 30,
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
		const source = built.scene.getObjectByName(
			`geometry-source:${mode.geometry.emitters[0].id}:0`,
		);
		if (!source) throw new Error("expected semantic geometry source");
		expect(source.userData.stageShaper).toEqual({
			supported: [true, true, false, false],
			insertions: [0.5, 0, 0, 0],
			anglesDegrees: [10, -45, 0, 0],
			moduleRotationDegrees: -90,
		});
		const module = source.getObjectByName("stage-shaper-module");
		if (!module) throw new Error("expected typed shaper module");
		expect(module.rotation.y).toBeCloseTo(-Math.PI / 2);
		expect(module.getObjectByName("stage-shaper-blade:1")?.visible).toBe(true);
		expect(module.getObjectByName("stage-shaper-blade:3")?.visible).toBe(false);
		disposeScene(built.scene);
	});
});
