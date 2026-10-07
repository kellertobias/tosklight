import { describe, expect, it } from "vitest";
import type { StoredPreset, VisualizationSnapshot } from "../../api/types";
import {
	presetFixtureCountLabel,
	presetFixtureCounts,
	resolvedValueIndex,
} from "./presetFixtureCounts";

const NO_GROUPS = new Map<string, readonly string[]>();

function snapshot(
	values: VisualizationSnapshot["values"],
): VisualizationSnapshot {
	return {
		revision: 1,
		generated_at: "",
		grand_master: 1,
		blackout: false,
		values,
	};
}

const beamPreset: Pick<StoredPreset, "values" | "group_values"> = {
	values: {
		"fixture-a": {
			zoom: { kind: "normalized", value: 0.5 },
			gobo: { kind: "discrete", value: "Dots" },
		},
		"fixture-b": {
			zoom: { kind: "normalized", value: 0.5 },
			gobo: { kind: "discrete", value: "Dots" },
		},
	},
};

function counts(
	preset: Pick<StoredPreset, "values" | "group_values" | "universal_values">,
	values: VisualizationSnapshot["values"] | null,
	groups = NO_GROUPS,
) {
	return presetFixtureCounts(
		preset,
		resolvedValueIndex(values ? snapshot(values) : null),
		groups,
	);
}

describe("Preset active / defined fixture counts", () => {
	it("counts zero active fixtures while nothing shows the stored look", () => {
		const result = counts(beamPreset, [
			{
				fixture_id: "fixture-a",
				attribute: "zoom",
				value: { kind: "normalized", value: 0.1 },
			},
		]);
		expect(result).toEqual({ active: 0, defined: 2 });
		expect(presetFixtureCountLabel(result)).toBe("0/2 fx");
	});

	it("counts zero active fixtures before any resolved values arrive", () => {
		expect(counts(beamPreset, null)).toEqual({ active: 0, defined: 2 });
	});

	it("counts a fixture active only when every stored attribute is effective", () => {
		const result = counts(beamPreset, [
			{
				fixture_id: "fixture-a",
				attribute: "zoom",
				value: { kind: "normalized", value: 0.50001 },
			},
			{
				fixture_id: "fixture-a",
				attribute: "gobo",
				value: { kind: "discrete", value: "Dots" },
			},
			{
				fixture_id: "fixture-b",
				attribute: "zoom",
				value: { kind: "normalized", value: 0.5 },
			},
		]);
		expect(presetFixtureCountLabel(result)).toBe("1/2 fx");
	});

	it("counts every defined fixture when all show the stored look", () => {
		const values = ["FIXTURE-A", "fixture-b"].flatMap((fixture_id) => [
			{
				fixture_id,
				attribute: "zoom",
				value: { kind: "normalized", value: 0.5 } as const,
			},
			{
				fixture_id,
				attribute: "gobo",
				value: { kind: "discrete", value: "Dots" } as const,
			},
		]);
		expect(presetFixtureCountLabel(counts(beamPreset, values))).toBe("2/2 fx");
	});

	it("shows 0/0 fx for a Preset that defines no fixture", () => {
		const result = counts({ values: {} }, [
			{
				fixture_id: "fixture-a",
				attribute: "zoom",
				value: { kind: "normalized", value: 0.5 },
			},
		]);
		expect(presetFixtureCountLabel(result)).toBe("0/0 fx");
	});

	it("resolves group values over ordered membership, spreading by position", () => {
		const groups = new Map([["group-1", ["fixture-c", "fixture-a"]]]);
		const preset = {
			values: {},
			group_values: {
				"group-1": { dimmer: { kind: "spread", value: [0, 1] } },
			},
		};
		const result = counts(
			preset,
			[
				{
					fixture_id: "fixture-c",
					attribute: "dimmer",
					value: { kind: "normalized", value: 0 },
				},
				{
					fixture_id: "fixture-a",
					attribute: "dimmer",
					value: { kind: "normalized", value: 0.4 },
				},
			],
			groups,
		);
		expect(result).toEqual({ active: 1, defined: 2 });
	});

	it("compares colors by their XYZ components", () => {
		const preset = {
			values: {
				"fixture-a": {
					color: { kind: "color_xyz", value: { x: 20, y: 30, z: 40 } },
				},
			},
		};
		expect(
			counts(preset, [
				{
					fixture_id: "fixture-a",
					attribute: "color",
					value: { kind: "color_xyz", value: { x: 20, y: 30, z: 40.0001 } },
				},
			]),
		).toEqual({ active: 1, defined: 1 });
	});

	describe("semantic family presets compare the requested intent", () => {
		type Effective = VisualizationSnapshot["values"][number]["value"];
		const angles = (tilt: number) =>
			({
				kind: "position",
				value: {
					kind: "angles",
					pan_degrees: { kind: "value", value: 0 },
					tilt_degrees: { kind: "value", value: tilt },
				},
			}) as unknown as Effective;
		// Stored with the show file's f64 spelling; the stream reports the compact f32 one.
		const yellow = (x: number, y: number, z: number) =>
			({
				kind: "color_program",
				value: {
					kind: "semantic",
					intent: {
						allocation: "preserve_recipe",
						base_xyz: { x, y, z },
						recipe: { amber: 0, approximate: false, rgb: [1, 1, 0], version: 1 },
						relative_output: 1,
						uv: { amount: 0 },
						white_blend: 0,
						white_target: { duv: 0, kelvin: 6500 },
					},
				},
			}) as unknown as Effective;
		const storedYellow = yellow(0.770032525062561, 0.9278250932693481, 0.13852590322494507);
		const effectiveYellow = yellow(0.7700325, 0.9278251, 0.1385259);
		const zoom = (degrees: number) =>
			({
				kind: "zoom",
				value: { opening_degrees: { kind: "value", value: degrees }, convention: "beam" },
			}) as unknown as Effective;
		const effective = (attribute: string, values: Record<string, Effective>) =>
			Object.entries(values).map(([fixture_id, value]) => ({
				fixture_id,
				attribute,
				value,
			}));
		const stored = (attribute: string, value: Effective) => ({
			values: {
				"fixture-a": { [attribute]: value },
				"fixture-b": { [attribute]: value },
			},
		});

		it("counts a Position preset on the fixtures whose requested Angles are the preset", () => {
			const down = stored("position", angles(-67.5));
			const up = stored("position", angles(45));
			const live = effective("position", {
				"fixture-a": angles(-67.5),
				"fixture-b": angles(-67.5),
			});
			expect(presetFixtureCountLabel(counts(down, live))).toBe("2/2 fx");
			// Negative control: a different stored Position stays inactive.
			expect(presetFixtureCountLabel(counts(up, live))).toBe("0/2 fx");
			const fixtureBElsewhere = effective("position", {
				"fixture-a": angles(-67.5),
				"fixture-b": angles(45),
			});
			expect(presetFixtureCountLabel(counts(down, fixtureBElsewhere))).toBe("1/2 fx");
		});

		it("matches a semantic Color across float spellings and rejects another colour", () => {
			const live = effective("color", {
				"fixture-a": effectiveYellow,
				"fixture-b": effectiveYellow,
			});
			expect(presetFixtureCountLabel(counts(stored("color", storedYellow), live))).toBe(
				"2/2 fx",
			);
			const red = yellow(0.4124564, 0.2126729, 0.0193339);
			expect(presetFixtureCountLabel(counts(stored("color", red), live))).toBe("0/2 fx");
			// A universal colour names no fixtures: it counts every fixture currently showing it.
			const universal = { values: {}, universal_values: { color: storedYellow } };
			expect(presetFixtureCountLabel(counts(universal, live))).toBe("Any · 2 fx");
			expect(
				presetFixtureCountLabel(
					counts({ values: {}, universal_values: { color: red } }, live),
				),
			).toBe("Any · 0 fx");
		});

		it("counts a Zoom preset by its requested opening", () => {
			const live = effective("zoom", { "fixture-a": zoom(20), "fixture-b": zoom(30) });
			expect(presetFixtureCountLabel(counts(stored("zoom", zoom(20)), live))).toBe("1/2 fx");
		});

		it("requires every stored family of a Mixed preset, alongside Intensity", () => {
			const mixed = {
				values: {
					"fixture-a": {
						intensity: { kind: "normalized", value: 1 },
						position: angles(-67.5),
					},
				},
			};
			const intensity = { values: { "fixture-a": { intensity: { kind: "normalized", value: 1 } } } };
			const live = [
				...effective("position", { "fixture-a": angles(45) }),
				...effective("intensity", { "fixture-a": { kind: "normalized", value: 1 } as Effective }),
			];
			expect(presetFixtureCountLabel(counts(mixed, live))).toBe("0/1 fx");
			expect(presetFixtureCountLabel(counts(intensity, live))).toBe("1/1 fx");
			const showing = [
				...effective("position", { "fixture-a": angles(-67.5) }),
				...effective("intensity", { "fixture-a": { kind: "normalized", value: 1 } as Effective }),
			];
			expect(presetFixtureCountLabel(counts(mixed, showing))).toBe("1/1 fx");
		});
	});
});
