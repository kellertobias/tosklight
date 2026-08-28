import { describe, expect, it } from "vitest";
import {
	depthFraction,
	fractionDepth,
	previewBounds,
	previewLabel,
	previewLateral,
	previewMarks,
} from "./depthPreview";
import type { CadEntity } from "./types";

function entity(id: string, x: number, y: number, z: number): CadEntity {
	return {
		id,
		kind: "fixture",
		name: id,
		positionMillimetres: [x, y, z],
		rotationDegrees: [0, 0, 0],
		sizeMillimetres: [100, 100, 100],
	} as unknown as CadEntity;
}

describe("the depth preview", () => {
	it("looks along the axis the view cannot see", () => {
		// Looking down cuts height, so the companion picture is an elevation across the stage.
		expect(previewLateral([1_000, 2_000, 3_000], "top_down")).toBe(1_000);
		expect(previewLabel("top_down")).toBe("Elevation");
		// Looking along the rig cuts a ground axis, so the companion is the plan.
		expect(previewLateral([1_000, 2_000, 3_000], "left_to_right")).toBe(2_000);
		expect(previewLabel("left_to_right")).toBe("Plan");
		expect(previewLateral([1_000, 2_000, 3_000], "front_to_back")).toBe(1_000);
	});

	it("places every element by its depth and its companion axis", () => {
		const marks = previewMarks(
			[entity("a", 0, 500, 0), entity("b", 4_000, 1_500, 0)],
			"left_to_right",
		);
		expect(marks).toEqual([
			{ id: "a", depth: 0, lateral: 500 },
			{ id: "b", depth: 4_000, lateral: 1_500 },
		]);
	});

	it("pads its span so an element on the edge is not cut in half", () => {
		const bounds = previewBounds(previewMarks([entity("a", 0, 0, 0)], "top_down"));
		expect(bounds.minDepth).toBeLessThan(0);
		expect(bounds.maxDepth).toBeGreaterThan(0);
	});

	it("gives an empty drawing a span its cuts can still be dragged along", () => {
		const bounds = previewBounds([]);
		expect(bounds.maxDepth).toBeGreaterThan(bounds.minDepth);
		expect(depthFraction(bounds.minDepth, bounds)).toBe(0);
		expect(depthFraction(bounds.maxDepth, bounds)).toBe(1);
	});

	it("maps a depth to its place across the preview and back again", () => {
		const bounds = { minDepth: 0, maxDepth: 1_000, minLateral: 0, maxLateral: 1 };
		expect(depthFraction(250, bounds)).toBeCloseTo(0.25);
		expect(fractionDepth(0.25, bounds)).toBeCloseTo(250);
		// A drag that leaves the preview stops at its edge rather than running off.
		expect(fractionDepth(-3, bounds)).toBe(0);
		expect(fractionDepth(4, bounds)).toBe(1_000);
	});
});
