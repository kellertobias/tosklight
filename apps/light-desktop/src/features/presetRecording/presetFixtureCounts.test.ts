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
	preset: Pick<StoredPreset, "values" | "group_values">,
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
		expect(presetFixtureCountLabel(result)).toBe("0 / 2");
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
		expect(presetFixtureCountLabel(result)).toBe("1 / 2");
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
		expect(presetFixtureCountLabel(counts(beamPreset, values))).toBe("2 / 2");
	});

	it("shows 0 / 0 for a Preset that defines no fixture", () => {
		const result = counts({ values: {} }, [
			{
				fixture_id: "fixture-a",
				attribute: "zoom",
				value: { kind: "normalized", value: 0.5 },
			},
		]);
		expect(presetFixtureCountLabel(result)).toBe("0 / 0");
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
});
