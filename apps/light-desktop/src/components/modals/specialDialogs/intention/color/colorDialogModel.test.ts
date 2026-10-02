import { describe, expect, it } from "vitest";
import type { FamilyEncoderPagesSnapshot } from "../../../../../api/familyEncoderModels";
import {
	CORE_COLOR_DESCRIPTORS,
	type ColorValueEntry,
	colorComponentChange,
	colorControlLimits,
	colorDescriptors,
	colorDialogVariant,
	encoderAreaFits,
	hueAlong,
	hueRangeSamples,
	hueTravel,
	isPendingEndpoint,
	mediaPreviewPixel,
	requestedColorValues,
} from "./colorDialogModel";

const D = CORE_COLOR_DESCRIPTORS;

function semantic(
	fixtureId: string,
	overrides: Partial<{
		rgb: [number, number, number];
		white_blend: number;
		kelvin: number;
		duv: number;
		uv: number;
		spreads: { component: "hue" | "saturation" | "white_blend" | "temperature" | "duv"; points: number[] }[];
	}> = {},
): ColorValueEntry {
	return {
		fixtureId,
		attribute: "color",
		value: {
			kind: "color_program",
			value: {
				kind: "semantic",
				intent: {
					base_xyz: { x: 0.4, y: 0.2, z: 0.02 },
					recipe: { version: 1, rgb: overrides.rgb ?? [1, 0, 0], amber: 0, approximate: false },
					white_blend: overrides.white_blend ?? 0,
					white_target: { kelvin: overrides.kelvin ?? 6500, duv: overrides.duv ?? 0 },
					uv: { amount: overrides.uv ?? 0 },
					relative_output: 1,
					allocation: "preserve_recipe",
					spreads: overrides.spreads,
				},
			},
		},
	};
}

describe("hue travel", () => {
	it("takes the shortest arc across 0°", () => {
		expect(hueTravel(350, 10)).toBe(20);
		expect(hueTravel(10, 350)).toBe(-20);
		expect(hueAlong([350, 10], 0.5)).toBe(0);
		const samples = hueRangeSamples([350, 10], 5);
		expect(samples).toEqual([350, 355, 0, 5, 10]);
		expect(samples.every((hue) => hue >= 350 || hue <= 10)).toBe(true);
	});

	it("resolves an exact 180° tie clockwise, towards increasing hue", () => {
		expect(hueTravel(0, 180)).toBe(180);
		expect(hueTravel(180, 0)).toBe(180);
		expect(hueAlong([180, 0], 0.5)).toBe(270);
		expect(hueAlong([90, 270], 0.5)).toBe(180);
	});

	it("keeps equal endpoints constant", () => {
		expect(hueRangeSamples([120, 120], 4)).toEqual([120, 120, 120, 120]);
	});
});

describe("gesture edits", () => {
	it("sends an ordinary edit as one component set in descriptor units", () => {
		expect(colorComponentChange("white_blend", 40, undefined, D)).toEqual({
			component: "white_blend",
			operation: { kind: "set", value: { kind: "value", value: 0.4 } },
		});
		expect(colorComponentChange("temperature", 3200, undefined, D).operation).toEqual({
			kind: "set",
			value: { kind: "value", value: 3200 },
		});
	});

	it("keeps a descending range in touched order as a spread", () => {
		expect(colorComponentChange("saturation", 80, [80, 20], D).operation).toEqual({
			kind: "set",
			value: { kind: "spread", value: [0.8, 0.2] },
		});
		expect(colorComponentChange("temperature", 9000, [9000, 3000], D).operation).toEqual({
			kind: "set",
			value: { kind: "spread", value: [9000, 3000] },
		});
	});

	it("sends hue endpoints as touched; the shortest arc is the server's spread rule", () => {
		expect(colorComponentChange("hue", 350, [350, 10], D).operation).toEqual({
			kind: "set",
			value: { kind: "spread", value: [350, 10] },
		});
	});

	it("shows but never writes a pending first endpoint", () => {
		expect(isPendingEndpoint(true, undefined)).toBe(true);
		expect(isPendingEndpoint(true, [10, 20])).toBe(false);
		expect(isPendingEndpoint(false, undefined)).toBe(false);
	});
});

describe("descriptors and limits", () => {
	it("uses the published family-page descriptors and completes the rest from the core table", () => {
		const published = { ...D.white_blend, step: 0.05 };
		const snapshot = {
			families: [
				{
					family: "color",
					pages: [
						{
							number: 1,
							label: "Color",
							slots: [
								{
									kind: "component",
									id: "color.white_blend",
									label: "White Blend",
									component: { kind: "color", component: "white_blend" },
									descriptor: published,
									limits_source: "descriptor",
									edit: "scalar",
									fixture_ids: ["a"],
								},
								null,
								null,
								null,
							],
						},
					],
				},
			],
		} as unknown as FamilyEncoderPagesSnapshot;
		const descriptors = colorDescriptors(snapshot);
		expect(descriptors.white_blend).toBe(published);
		expect(descriptors.hue).toBe(D.hue);
		expect(colorControlLimits(descriptors.white_blend)).toEqual({ min: 0, max: 100, step: 5 });
		expect(colorControlLimits(D.hue)).toEqual({ min: 0, max: 359, step: 1 });
		expect(colorControlLimits(D.duv)).toEqual({ min: -0.03, max: 0.03, step: 0.001 });
	});
});

describe("requested values", () => {
	it("reads hue and saturation from the recipe and ranges from stored spreads", () => {
		const values = requestedColorValues(
			[
				semantic("a", {
					rgb: [0, 1, 1],
					white_blend: 0.25,
					uv: 0.4,
					spreads: [
						{ component: "hue", points: [350, 10] },
						{ component: "white_blend", points: [0.8, 0.2] },
					],
				}),
			],
			["a"],
			D,
		);
		expect(values.ranges.hue).toEqual([350, 10]);
		expect(values.ranges.white_blend).toEqual([80, 20]);
		expect(values.hue).toBe(350);
		expect(values.saturation).toBe(100);
		expect(values.white_blend).toBe(80);
		expect(values.uv).toBe(40);
		expect(values.programmed).toBe(true);
	});

	it("reads a fixture-addressed range back from its per-fixture values in selection order", () => {
		// The backend resolves a fixture-addressed spread by rank: 80 → 50 → 20 is stored per fixture.
		const entries = [
			semantic("c", { white_blend: 0.2, rgb: [0, 1, 0] }),
			semantic("a", { white_blend: 0.8, rgb: [1, 0, 0] }),
			semantic("b", { white_blend: 0.5, rgb: [1, 1, 0] }),
		];
		const values = requestedColorValues(entries, ["a", "b", "c"], D);
		expect(values.ranges.white_blend?.map((value) => Math.round(value * 1e6) / 1e6)).toEqual([80, 20]);
		expect(values.white_blend).toBeCloseTo(80, 6);
		// Hue 0° → 60° → 120°: an even arc, so Hue reads as its range too.
		expect(values.ranges.hue).toEqual([0, 120]);
		// Reversing the selection reverses the range.
		expect(requestedColorValues(entries, ["c", "b", "a"], D).ranges.white_blend?.map(Math.round)).toEqual([20, 80]);
		// Uneven steps, a missing value or equal values are no range.
		const uneven = [semantic("a", { white_blend: 0.8 }), semantic("b", { white_blend: 0.7 }), semantic("c", { white_blend: 0.2 })];
		expect(requestedColorValues(uneven, ["a", "b", "c"], D).ranges.white_blend).toBeUndefined();
		expect(requestedColorValues(entries.slice(1), ["a", "b", "c"], D).ranges.white_blend).toBeUndefined();
		const equal = [semantic("a", { white_blend: 0.4 }), semantic("b", { white_blend: 0.4 })];
		expect(requestedColorValues(equal, ["a", "b"], D).ranges).toEqual({});
	});

	it("reads a resolved Hue range along the shortest arc across 0°", () => {
		const hueRgb = (hue: number): [number, number, number] => {
			const sector = (hue % 360) / 60;
			const x = 1 - Math.abs((sector % 2) - 1);
			return ([[1, x, 0], [x, 1, 0], [0, 1, x], [0, x, 1], [x, 0, 1], [1, 0, x]] as const)[Math.floor(sector)] as unknown as [number, number, number];
		};
		const entries = [350, 0, 10].map((hue, index) => semantic(String(index), { rgb: hueRgb(hue) }));
		const range = requestedColorValues(entries, ["0", "1", "2"], D).ranges.hue;
		expect(range?.[0]).toBeCloseTo(350, 6);
		expect(range?.[1]).toBeCloseTo(10, 6);
	});

	it("flags a mixed selection and ignores fixtures outside it", () => {
		const entries = [semantic("a"), semantic("b", { rgb: [0, 0, 1] }), semantic("c", { rgb: [0, 1, 0] })];
		expect(requestedColorValues(entries, ["a", "b"], D).mixed).toBe(true);
		expect(requestedColorValues(entries, ["a"], D).mixed).toBe(false);
		expect(requestedColorValues([], ["a"], D).programmed).toBe(false);
	});
});

describe("variant and encoder-area budget", () => {
	it("uses the Media dialog only when every selected owner is a Media head", () => {
		expect(colorDialogVariant(["l1", "l2"], ["l1", "l2"])).toBe("media");
		expect(colorDialogVariant(["lamp", "layer"], ["layer"])).toBe("lamp");
		expect(colorDialogVariant([], [])).toBe("lamp");
	});

	it("fits the compact dialog only at 680×210 or more of the measured area", () => {
		expect(encoderAreaFits({ width: 680, height: 210 })).toBe(true);
		expect(encoderAreaFits({ width: 679, height: 400 })).toBe(false);
		expect(encoderAreaFits({ width: 1200, height: 209 })).toBe(false);
	});
});

describe("Media preview (presentation of the shared tint and White Blend)", () => {
	const red = [1, 0.2, 0.2];
	const neutral = [1, 1, 1];
	it("desaturates with White Blend while the tint is retained", () => {
		const none = mediaPreviewPixel(red, neutral, 0);
		const half = mediaPreviewPixel(red, neutral, 0.5);
		const full = mediaPreviewPixel(red, neutral, 1);
		expect(none[0]).toBeCloseTo(1, 5);
		expect(half[0] - half[1]).toBeLessThan(none[0] - none[1]);
		expect(full[0]).toBeCloseTo(full[1], 5);
		expect(full[1]).toBeCloseTo(full[2], 5);
		const tinted = mediaPreviewPixel(red, [1, 0, 0], 1);
		expect(tinted[1]).toBe(0);
		expect(tinted[0]).toBeGreaterThan(0);
	});

	it("gives a red image under a 100 % red tint and needs RGB at 100 % for neutral", () => {
		const white = [1, 1, 1];
		expect(mediaPreviewPixel(white, [1, 0, 0], 0).map((value) => Number(value.toFixed(6)))).toEqual([1, 0, 0]);
		const neutralOut = mediaPreviewPixel(white, neutral, 0);
		expect(neutralOut.map((value) => Number(value.toFixed(6)))).toEqual([1, 1, 1]);
		const short = mediaPreviewPixel(white, [1, 1, 0.9], 0);
		expect(short[2]).toBeLessThan(1);
	});
});
