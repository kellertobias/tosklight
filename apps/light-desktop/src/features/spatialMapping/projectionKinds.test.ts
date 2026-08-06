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

	it("offers only the fields the chosen kind uses", () => {
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
			"Rotation X",
			"Rotation Y",
			"Rotation Z",
			"Start angle",
		]);

		// A sphere has no axis, so it takes two angles instead of three rotations.
		const spherical = withProjectionKind(planar, "spherical");
		expect(labels(spherical)).toEqual([
			"Position X",
			"Position Y",
			"Position Z",
			"Centre azimuth",
			"Centre elevation",
		]);
	});

	it("carries the centre point across a kind change and drops the preset", () => {
		const cylindrical = withProjectionKind(planar, "cylindrical");
		expect(cylindrical.anchor).toEqual({ x: 1, y: 2, z: 3 });
		expect(cylindrical.preset).toBeNull();
		// Only a planar projection looks along a direction.
		expect(supportsPreset(cylindrical)).toBe(false);
	});

	it("returns the same projection when the kind is unchanged", () => {
		expect(withProjectionKind(planar, "planar")).toBe(planar);
	});

	it("edits the value the field names", () => {
		const cylindrical = withProjectionKind(planar, "cylindrical");
		const fields = projectionFields(cylindrical);
		const startAngle = fields.find((field) => field.label === "Start angle");
		expect(startAngle?.apply(45).start_angle_degrees).toBe(45);
		const rotationY = fields.find((field) => field.label === "Rotation Y");
		expect(rotationY?.apply(90).axis_rotation).toEqual({ x: 0, y: 90, z: 0 });
	});
});
