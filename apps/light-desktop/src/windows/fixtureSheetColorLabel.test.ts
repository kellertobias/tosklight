import { describe, expect, it } from "vitest";
import type { AttributeValue, VisualizationSnapshot } from "../api/types";
import { srgbToXyz } from "../features/presetPreview/colorDisplay";
import { fixtureSheetColorLabel } from "./fixtureSheetColorLabel";
import { fixtureSheetSnapshotsEqual } from "./fixtureSheetProjection";
const color = (r: number, g: number, b: number): AttributeValue => ({ kind: "color_xyz", value: srgbToXyz(r, g, b) });
describe("Fixture Sheet color labels", () => {
	it.each([
		[1, 0, 0, "Red"], [0, 1, 0, "Green"], [0, 0, 1, "Blue"],
		[1, 1, 1, "White"], [0, 0, 0, "Black"], [1, 0.5, 0, "Orange"],
		[0, 1, 1, "Cyan"], [1, 0, 1, "Magenta"],
	] as const)("names requested RGB %s,%s,%s approximately as <%s>", (r, g, b, name) => {
		expect(fixtureSheetColorLabel(color(r, g, b))).toBe(`<${name}>`);
	});
	it("names semantic Color intent and keeps UV-only distinct from a visible violet", () => {
		const semantic = (r: number, g: number, b: number, uv = 0): AttributeValue => ({
			kind: "color_program", value: { kind: "semantic", intent: {
				base_xyz: srgbToXyz(r, g, b), recipe: { version: 1, rgb: [r, g, b], amber: 0, approximate: false },
				white_blend: 0, white_target: { kelvin: 6500, duv: 0 }, uv: { amount: uv },
				relative_output: 1, allocation: "preserve_recipe", wheel_constraints: [], spreads: [],
			} },
		});
		expect(fixtureSheetColorLabel(semantic(1, 0, 0))).toBe("<Red>");
		expect(fixtureSheetColorLabel(semantic(0, 0, 0, 1))).toBe("<UV-only>");
	});
	it("uses actual Preset provenance, while an identical literal stays an approximate color name", () => {
		const literal = color(1, 0, 0);
		expect(fixtureSheetColorLabel(literal, "Opening Ruby")).toBe("Opening Ruby");
		expect(fixtureSheetColorLabel(literal)).toBe("<Red>");
		expect(fixtureSheetColorLabel(literal, "")).toBe("<Red>");
	});
	it("treats a Preset rename or provenance removal as a sheet update even if numeric color stays identical", () => {
		const snapshot: VisualizationSnapshot = { revision: 1, generated_at: "now", grand_master: 1, blackout: false,
			values: [{ fixture_id: "fixture", attribute: "color", value: color(1, 0, 0) }],
			color_preset_names: [{ fixture_id: "fixture", preset_instance_id: "preset", name: "Ruby" }],
		};
		expect(fixtureSheetSnapshotsEqual(snapshot, { ...snapshot, color_preset_names: [{ fixture_id: "fixture", preset_instance_id: "preset", name: "Red look" }] })).toBe(false);
		expect(fixtureSheetSnapshotsEqual(snapshot, { ...snapshot, color_preset_names: [] })).toBe(false);
	});
});
