import { describe, expect, it } from "vitest";
import type { PatchedFixture } from "../../api/types";
import {
	colorRangeMutation,
	hsvToRgb,
	selectedFixtureIdsSupportingAttribute,
} from "./specialColor";

const red = { hue: 0, saturation: 1, brightness: 1 };
const blue = { hue: 2 / 3, saturation: 1, brightness: 1 };

describe("color range gesture intent", () => {
	it("ships the ordered selection, endpoints, and signed hue travel unchanged", () => {
		const mutation = colorRangeMutation(
			["fixture-b", "fixture-a"],
			red,
			blue,
			-1 / 3,
			0.8,
			2_000,
		);
		expect(mutation).toEqual({
			action: "set_selection_color_range",
			fixtureIds: ["fixture-b", "fixture-a"],
			start: { hue: 0, saturation: 1 },
			end: { hue: 2 / 3, saturation: 1 },
			hueTravel: -1 / 3,
			brightness: 0.8,
			timing: { fade: true, fadeMillis: 2_000, delayMillis: null },
		});
	});

	it("snaps a drag released back on its start color to the exact closed loop", () => {
		const nearStart = { hue: 0.004, saturation: 0.995, brightness: 1 };
		const mutation = colorRangeMutation(
			["a", "b", "c"],
			red,
			nearStart,
			0.997,
			1,
			undefined,
		);
		expect(mutation.action).toBe("set_selection_color_range");
		if (mutation.action !== "set_selection_color_range") return;
		expect(mutation.hueTravel).toBe(1);
		expect(mutation.end).toEqual({ hue: 0, saturation: 1 });
		expect(mutation.timing.fadeMillis).toBe(3_000);
	});

	it("keeps near-whole travel open when the release point is a different color", () => {
		const mutation = colorRangeMutation(
			["a", "b"],
			red,
			blue,
			0.999,
			1,
			undefined,
		);
		if (mutation.action !== "set_selection_color_range") return;
		expect(mutation.hueTravel).toBeCloseTo(0.999);
		expect(mutation.end).toEqual({ hue: 2 / 3, saturation: 1 });
	});
});

describe("swatch conversion", () => {
	it("matches the primary corners of the picker", () => {
		expect(hsvToRgb(red)).toEqual([1, 0, 0]);
		expect(hsvToRgb({ hue: 1 / 3, saturation: 1, brightness: 1 })).toEqual([
			0, 1, 0,
		]);
		expect(hsvToRgb(blue)).toEqual([0, 0, 1]);
	});
});

describe("independent whole-color attributes", () => {
	it("targets Tint only at selected fixture or logical-head identities that support it", () => {
		const fixture = {
			fixture_id: "fixture-a",
			definition: {
				heads: [
					{ parameters: [{ attribute: "color.tint" }] },
					{ parameters: [{ attribute: "color.temperature" }] },
				],
			},
			logical_heads: [
				{ fixture_id: "fixture-a:head-1", head_index: 0 },
				{ fixture_id: "fixture-a:head-2", head_index: 1 },
			],
		} as unknown as PatchedFixture;

		expect(
			selectedFixtureIdsSupportingAttribute(
				[fixture],
				["fixture-a:head-1", "fixture-a:head-2"],
				["color.tint", "fixture.tint"],
			),
		).toEqual(["fixture-a:head-1"]);
		expect(
			selectedFixtureIdsSupportingAttribute(
				[fixture],
				["fixture-a"],
				["color.tint", "fixture.tint"],
			),
		).toEqual(["fixture-a"]);
	});

	it("targets media grayscale only at compatible selected logical heads", () => {
		const fixture = {
			fixture_id: "fixture-a",
			definition: {
				heads: [
					{
						parameters: [
							{ attribute: "color" },
							{ attribute: "media.grayscale" },
						],
					},
					{ parameters: [{ attribute: "color" }] },
				],
			},
			logical_heads: [
				{ fixture_id: "fixture-a:head-1", head_index: 0 },
				{ fixture_id: "fixture-a:head-2", head_index: 1 },
			],
		} as unknown as PatchedFixture;

		expect(
			selectedFixtureIdsSupportingAttribute(
				[fixture],
				["fixture-a:head-1", "fixture-a:head-2"],
				["media.grayscale"],
			),
		).toEqual(["fixture-a:head-1"]);
	});
});
