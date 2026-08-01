import { describe, expect, it } from "vitest";
import { PLANNED_DEMO_FIXTURES } from "../../support/plannedDemoManifest";
import { installPlannedDemoPresets } from "../../support/plannedDemoPresets";

describe("Plan 76 preset library", () => {
	it("defines the exact 13 Color, 7 Position, and 10 Beam presets", async () => {
		const writes: Array<{ id: string; body: any }> = [];
		const fixtures = PLANNED_DEMO_FIXTURES.map((fixture) => ({
			fixture_id: `fixture-${fixture.number}`,
			fixture_number: fixture.number,
			logical_heads: [],
		}));
		const result = await installPlannedDemoPresets(
			{
				seedShowObject: async (
					_showId: string,
					_kind: string,
					id: string,
					body: any,
				) => {
					writes.push({ id, body });
				},
			} as any,
			"show",
			fixtures,
		);
		expect(result).toEqual({ colors: 13, positions: 7, beam: 10 });
		expect(writes).toHaveLength(30);
		expect(
			writes
				.filter((write) => write.body.family === "Color")
				.map((write) => write.body.name),
		).toEqual([
			"Red",
			"Orange",
			"Yellow",
			"Lime",
			"Green",
			"Teal",
			"Cyan",
			"Light Blue",
			"Dark Blue",
			"Purple",
			"Magenta",
			"White",
			"Tungsten White",
		]);
		expect(
			writes.find((write) => write.body.name === "Red")?.body,
		).toMatchObject({ icon: "●", color: "#ff0000" });
		expect(
			writes.find((write) => write.body.name === "Light Blue")?.body,
		).toMatchObject({ icon: "●", color: "#40a6ff" });
		expect(
			writes.find((write) => write.body.name === "Tungsten White")?.body,
		).toMatchObject({ icon: "●", color: "#ff9e52" });
		expect(
			writes.find((write) => write.body.name === "Blind")?.body.values[
				"fixture-101"
			],
		).toHaveProperty("pan");
		expect(
			writes.find((write) => write.body.name === "Prism Rotation")?.body.values[
				"fixture-101"
			],
		).toHaveProperty("prism.prism_rotation");
	});
});
