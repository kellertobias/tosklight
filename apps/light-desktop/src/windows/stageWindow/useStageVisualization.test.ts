import { describe, expect, it } from "vitest";
import type { PatchedFixture } from "../../api/types";
import { stageFixtures3d } from "./useStageVisualization";

describe("stageFixtures3d", () => {
	it("preserves an explicit zero location instead of replacing it with fallback geometry", () => {
		const fixture = {
			fixture_id: "zero-location",
			fixture_number: 1,
			logical_heads: [],
			location: { x: 0, y: 0, z: 0 },
			rotation: { x: 0, y: 0, z: 0 },
			multipatch: [],
		} as unknown as PatchedFixture;

		expect(
			stageFixtures3d([fixture], { positions: {}, positions3d: {} })[0]
				.position,
		).toMatchObject({ x: 0, y: 0, z: 0 });
	});

	it("keeps installed appearance independent for the root and each physical copy", () => {
		const rootAppearance = {
			light_source: { type: "profile_default" as const },
			color_temperature_kelvin: 3_200,
			gel: { type: "open_white" as const },
			shaper_angles_degrees: [1, 2, 3, 4] as [number, number, number, number],
		};
		const copyAppearance = {
			...rootAppearance,
			color_temperature_kelvin: 10_000,
		};
		const fixture = {
			fixture_id: "fixture",
			logical_heads: [],
			installed_appearance: rootAppearance,
			shaper_angle: 15,
			multipatch: [
				{
					id: "copy",
					location: { x: 0, y: 0, z: 0 },
					rotation: { x: 0, y: 0, z: 0 },
					installed_appearance: copyAppearance,
					shaper_angle: -30,
				},
			],
		} as unknown as PatchedFixture;

		const [root, copy] = stageFixtures3d([fixture], {
			positions: {},
			positions3d: {},
		});
		expect(root.installedAppearance).toBe(rootAppearance);
		expect(root.shaperAngle).toBe(15);
		expect(copy.installedAppearance).toBe(copyAppearance);
		expect(copy.shaperAngle).toBe(-30);
	});
});
