import { describe, expect, it } from "vitest";
import {
	inheritsOrdering,
	shapeForOrdering,
	spatialMappingForOrdering,
} from "./phaseOrderingShape";

const liveGroup = { type: "live_group" as const, group_id: "front" };
const frozen = { type: "frozen_targets" as const, targets: ["fixture-1"] };

const inheritBoth = {
	projection: { type: "inherit" as const },
	shape: { type: "inherit" as const },
};

describe("phase ordering shape", () => {
	it("maps each ordering onto the shape it means", () => {
		expect(shapeForOrdering({ type: "grid_linear", angle_degrees: 45 })).toEqual(
			{ type: "grid", angle_degrees: 45, direction: "ascending" },
		);
		expect(
			shapeForOrdering({ type: "radial_in", center_x: 1, center_z: 2 }),
		).toEqual({ type: "radial", center_u: 1, center_v: 2, direction: "inward" });
		expect(shapeForOrdering({ type: "random_each_loop", seed: 9 })).toEqual({
			type: "random",
			seed: 9,
		});
		// Selection order ranks by the saved selection, so it uses no shape at all.
		expect(shapeForOrdering({ type: "selection" })).toBeNull();
	});

	it("reads as Inherit only when there is a Group to inherit from", () => {
		expect(inheritsOrdering(inheritBoth, liveGroup)).toBe(true);
		// Frozen targets have no Group, so the ordering is the Dynamic's own.
		expect(inheritsOrdering(inheritBoth, frozen)).toBe(false);
		// A Dynamic with no stored mapping is a legacy one whose ordering is authoritative.
		expect(inheritsOrdering(undefined, liveGroup)).toBe(false);
		expect(
			inheritsOrdering(
				{
					...inheritBoth,
					shape: {
						type: "replace",
						value: { type: "grid", angle_degrees: 0, direction: "ascending" },
					},
				},
				liveGroup,
			),
		).toBe(false);
	});

	it("moves the shape stage with the ordering and leaves the projection alone", () => {
		const projectionOverride = {
			projection: {
				type: "replace" as const,
				value: {
					anchor: { x: 1, y: 0, z: 0 },
					view_direction: { x: 0, y: 0, z: -1 },
					rotation_degrees: 0,
				},
			},
			shape: { type: "inherit" as const },
		};
		const next = spatialMappingForOrdering(projectionOverride, {
			type: "grid_linear",
			angle_degrees: 30,
		});
		expect(next.shape).toEqual({
			type: "replace",
			value: { type: "grid", angle_degrees: 30, direction: "ascending" },
		});
		// The Projection is a separate concern and must survive an ordering change.
		expect(next.projection).toEqual(projectionOverride.projection);

		expect(spatialMappingForOrdering(next, null).shape).toEqual({
			type: "inherit",
		});
	});
});
