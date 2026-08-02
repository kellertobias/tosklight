import { describe, expect, it } from "vitest";
import {
	dynamicMappingBaseLabel,
	dynamicSpatialDraft,
	INHERIT_SPATIAL_MAPPING,
	validateDynamicSpatialDraft,
} from "./dynamicSpatialDraft";

const projection = {
	anchor: { x: 0, y: 0, z: 0 },
	view_direction: { x: 0, y: 0, z: -1 },
	rotation_degrees: 0,
	preset: "top" as const,
};

describe("Dynamic spatial mapping draft", () => {
	it("canonicalizes absent and malformed stages to explicit inheritance", () => {
		expect(dynamicSpatialDraft(undefined)).toEqual(INHERIT_SPATIAL_MAPPING);
		expect(
			dynamicSpatialDraft({
				projection: { type: "replace", value: { future: true } },
				shape: { type: "future" },
			}),
		).toEqual(INHERIT_SPATIAL_MAPPING);
	});

	it("retains independent valid Projection and Shape overrides", () => {
		expect(
			dynamicSpatialDraft({
				projection: { type: "replace", value: projection },
				shape: {
					type: "replace",
					value: {
						type: "radial",
						center_u: 1,
						center_v: 2,
						direction: "inward",
					},
				},
			}),
		).toMatchObject({
			projection: { type: "replace", value: projection },
			shape: { type: "replace", value: { type: "radial" } },
		});
	});

	it("requires a complete spatial override when no inherited mapping exists", () => {
		expect(
			validateDynamicSpatialDraft(
				{
					projection: { type: "replace", value: projection },
					shape: { type: "inherit" },
				},
				false,
			),
		).toMatch(/both be overridden/);
		expect(
			validateDynamicSpatialDraft(
				{
					projection: { type: "replace", value: projection },
					shape: { type: "inherit" },
				},
				true,
			),
		).toBeNull();
	});

	it("treats Random as position-independent and ignores Projection completeness", () => {
		expect(
			validateDynamicSpatialDraft(
				{
					projection: { type: "inherit" },
					shape: { type: "replace", value: { type: "random", seed: 42 } },
				},
				false,
			),
		).toBeNull();
		expect(
			validateDynamicSpatialDraft(
				{
					projection: { type: "inherit" },
					shape: { type: "replace", value: { type: "random", seed: -1 } },
				},
				false,
			),
		).toMatch(/non-negative/);
	});

	it("labels saved live bindings without consulting current selection", () => {
		expect(
			dynamicMappingBaseLabel({ type: "live_group", group_id: "front" }),
		).toBe("Inherit group mapping · Group front");
		expect(dynamicMappingBaseLabel({ type: "targetless" })).toBe(
			"Selection order (no Group mapping)",
		);
	});
});
