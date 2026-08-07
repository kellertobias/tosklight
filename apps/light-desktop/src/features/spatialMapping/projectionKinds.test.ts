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
		// Planar reads no position, so it is not offered one.
		expect(labels(planar)).toEqual([
			"Direction X",
			"Direction Y",
			"Direction Z",
			"Rotation",
		]);

		const cylindrical = withProjectionKind(planar, "cylindrical");
		expect(labels(cylindrical)).toEqual([
			"Position X",
			"Position Y",
			"Position Z",
			"Direction X",
			"Direction Y",
			"Direction Z",
			"Rotation",
		]);

		// A roll about the centre of a spherical spread does not move it, so there is none.
		const spherical = withProjectionKind(planar, "spherical");
		expect(labels(spherical)).toEqual([
			"Position X",
			"Position Y",
			"Position Z",
			"Direction X",
			"Direction Y",
			"Direction Z",
		]);
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
		const directionY = fields.find((field) => field.label === "Direction Y");
		expect(directionY?.apply(0.5).view_direction).toEqual({
			x: 0,
			y: 0.5,
			z: -1,
		});
	});
});
