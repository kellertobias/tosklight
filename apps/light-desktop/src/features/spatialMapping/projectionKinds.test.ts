import { describe, expect, it } from "vitest";
import type { SpatialProjection } from "./contracts";
import {
	projectionFields,
	projectionKind,
	supportsPreset,
	withProjectionKind,
} from "./projectionKinds";

const planar: SpatialProjection = {
	anchor: { x: 1, y: 2, z: 3 },
	view_direction: { x: 0, y: 0, z: -1 },
	rotation_degrees: 0,
	preset: "top",
};

function labels(projection: SpatialProjection) {
	return projectionFields(projection).map((field) => field.label);
}

describe("projection kinds", () => {
	it("reads a stored projection with no kind as planar", () => {
		expect(projectionKind(planar)).toBe("planar");
		expect(supportsPreset(planar)).toBe(true);
	});

	it("offers a position and a direction, and nothing else", () => {
		// Planar reads no position, so it is not offered one. It keeps components because a view
		// preset names one and the numbers are how a preset reads back.
		expect(labels(planar)).toEqual([
			"Direction X",
			"Direction Y",
			"Direction Z",
			"Rotation",
		]);

		// The placed kinds are aimed by the two turns that aim them, then rolled about the result.
		const placed = ["Position X", "Position Y", "Position Z"];
		expect(labels(withProjectionKind(planar, "cylindrical"))).toEqual([
			...placed,
			"Azimuth",
			"Elevation",
			"Rotation",
		]);
		expect(labels(withProjectionKind(planar, "spherical"))).toEqual([
			...placed,
			"Azimuth",
			"Elevation",
			"Rotation",
		]);
	});

	it("aims a direction by azimuth and elevation", () => {
		const cylindrical = withProjectionKind(planar, "cylindrical");
		const fields = projectionFields(cylindrical);
		// Straight down is the bottom pole, whatever the azimuth reads there.
		expect(
			fields.find((field) => field.label === "Elevation")?.value,
		).toBeCloseTo(-90);

		const level = fields.find((field) => field.label === "Elevation")?.apply(0);
		expect(level?.view_direction.z).toBeCloseTo(0);
		const swung = projectionFields(level as SpatialProjection)
			.find((field) => field.label === "Azimuth")
			?.apply(90);
		expect(swung?.view_direction.x).toBeCloseTo(0);
		expect(swung?.view_direction.y).toBeCloseTo(1);
	});

	it("carries the position and direction across a kind change and drops the preset", () => {
		const cylindrical = withProjectionKind(planar, "cylindrical");
		expect(cylindrical.anchor).toEqual({ x: 1, y: 2, z: 3 });
		expect(cylindrical.view_direction).toEqual({ x: 0, y: 0, z: -1 });
		expect(cylindrical.preset).toBeNull();
		// Only a planar projection looks along a direction.
		expect(supportsPreset(cylindrical)).toBe(false);
	});

	it("never leaves a projection without a direction to orient it", () => {
		const flat = { ...planar, view_direction: { x: 0, y: 0, z: 0 } };
		expect(withProjectionKind(flat, "spherical").view_direction).toEqual({
			x: 0,
			y: 0,
			z: -1,
		});
	});

	it("returns the same projection when the kind is unchanged", () => {
		expect(withProjectionKind(planar, "planar")).toBe(planar);
	});

	it("edits the value the field names", () => {
		const cylindrical = withProjectionKind(planar, "cylindrical");
		const fields = projectionFields(cylindrical);
		const rotation = fields.find((field) => field.label === "Rotation");
		expect(rotation?.apply(45).rotation_degrees).toBe(45);
		const positionY = fields.find((field) => field.label === "Position Y");
		expect(positionY?.apply(9).anchor).toEqual({ x: 1, y: 9, z: 3 });
	});
});
