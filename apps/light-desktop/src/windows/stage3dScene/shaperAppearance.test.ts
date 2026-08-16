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
	it("draws all four installed blades and independent module and bracket angles per physical instance", () => {
		const profile = blankFixtureProfile();
		const mode = profile.modes[0];
		mode.geometry = geometryTemplate("fixed", [mode.heads[0].id]);
		mode.channels = [
			{ ...blankChannel(mode), attribute: "intensity" },
			...([1, 2, 3, 4] as const).map((blade) => ({
				...blankChannel(mode),
				attribute: `shaper.blade.${blade}.position`,
			})),
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
			],
		};
		const appearance = (
			shaper_angles_degrees: [number, number, number, number],
		) => ({
			light_source: { type: "profile_default" as const },
			luminous_output_lumens: null,
			color_temperature_kelvin: null,
			gel: { type: "open_white" as const },
			shaper_angles_degrees,
		});
		const built = buildStageScene(
			[
				{
					fixture,
					installedAppearance: appearance([10, 20, 30, 40]),
					shaperAngle: 15,
					bracketAngle: 25,
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
				{
					fixture,
					instanceId: "physical-copy",
					installedAppearance: appearance([-11, -22, -33, -44]),
					shaperAngle: -30,
					bracketAngle: -20,
					index: 1,
					position: {
						x: 1,
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

		const expected = [
			{
				instanceId: profile.id,
				angles: [10, 20, 30, 40],
				module: 15,
				bracket: 25,
			},
			{
				instanceId: "physical-copy",
				angles: [-11, -22, -33, -44],
				module: -30,
				bracket: -20,
			},
		] as const;
		for (const instance of expected) {
			const root = built.scene.getObjectByName(
				`fixture:${profile.id}:${instance.instanceId}`,
			);
			if (!root)
				throw new Error(`expected fixture root ${instance.instanceId}`);
			expect(root.rotation.x).toBeCloseTo((instance.bracket * Math.PI) / 180);
			const source = root.getObjectByName(
				`geometry-source:${mode.geometry.emitters[0].id}:0`,
			);
			if (!source) throw new Error("expected semantic geometry source");
			expect(source.userData.stageShaper).toMatchObject({
				supported: [true, true, true, true],
				anglesDegrees: instance.angles,
				moduleRotationDegrees: instance.module,
			});
			const module = source.getObjectByName("stage-shaper-module");
			if (!module) throw new Error("expected typed shaper module");
			expect(module.rotation.y).toBeCloseTo((instance.module * Math.PI) / 180);
			instance.angles.forEach((angle, index) => {
				const blade = module.getObjectByName(`stage-shaper-blade:${index + 1}`);
				expect(blade?.visible).toBe(true);
				expect(blade?.rotation.y).toBeCloseTo(
					((angle + (index < 2 ? 0 : 90)) * Math.PI) / 180,
				);
			});
		}
		disposeScene(built.scene);
	});

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
						luminous_output_lumens: null,
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
