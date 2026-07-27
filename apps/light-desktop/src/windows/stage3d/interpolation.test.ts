import { describe, expect, it } from "vitest";
import type { VisualizationSnapshot } from "../../api/types";
import { interpolateVisualizationSnapshot } from "./interpolation";

describe("interpolateVisualizationSnapshot", () => {
	it("moves continuous values toward but never beyond the authoritative sample", () => {
		const from = snapshot(1, false, [
			normalized("intensity", 0),
			color("color", 0, 0.2, 0.4),
		]);
		const to = snapshot(2, false, [
			normalized("intensity", 1),
			color("color", 1, 0.6, 0.2),
		]);

		const halfway = interpolateVisualizationSnapshot(from, to, 0.5);
		expect(halfway.values[0]).toEqual(normalized("intensity", 0.5));
		expect(halfway.values[1]?.value).toMatchObject({
			kind: "color_xyz",
			value: {
				x: 0.5,
				y: 0.4,
				z: expect.closeTo(0.3),
			},
		});
		expect(interpolateVisualizationSnapshot(from, to, 4)).toBe(to);
	});

	it("applies discrete values and newly appearing attributes immediately", () => {
		const from = snapshot(1, false, [
			{ ...normalized("gobo", 0), value: { kind: "discrete", value: "open" } },
		]);
		const to = snapshot(2, false, [
			{ ...normalized("gobo", 0), value: { kind: "discrete", value: "star" } },
			normalized("zoom", 0.75),
		]);

		expect(interpolateVisualizationSnapshot(from, to, 0).values).toEqual(
			to.values,
		);
	});

	it("applies blackout immediately rather than fading through it", () => {
		const from = snapshot(1, false, [normalized("intensity", 1)]);
		const to = snapshot(2, true, [normalized("intensity", 0)]);

		expect(interpolateVisualizationSnapshot(from, to, 0)).toBe(to);
	});
});

function snapshot(
	revision: number,
	blackout: boolean,
	values: VisualizationSnapshot["values"],
): VisualizationSnapshot {
	return {
		revision,
		generated_at: `2026-07-27T00:00:0${revision}Z`,
		grand_master: 1,
		blackout,
		preload: false,
		values,
		profile_output_values: [],
	};
}

function normalized(attribute: string, value: number) {
	return {
		fixture_id: "fixture-1",
		attribute,
		value: { kind: "normalized" as const, value },
	};
}

function color(attribute: string, x: number, y: number, z: number) {
	return {
		fixture_id: "fixture-1",
		attribute,
		value: { kind: "color_xyz" as const, value: { x, y, z } },
	};
}
