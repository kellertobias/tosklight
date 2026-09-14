import { describe, expect, it } from "vitest";
import type { PatchedFixture } from "../../wire";
import {
	formatModelScale,
	hasModelScale,
	modelScaleChange,
	modelScaleOf,
} from "./modelScale";

function fixture(
	policy: "dmx" | "visual_only",
	model_scale?: number | null,
	crowd = false,
) {
	return {
		model_scale,
		definition: {
			profile_snapshot: { patch_policy: policy, crowd: crowd ? {} : null },
		},
	} as unknown as PatchedFixture;
}

describe("model scale", () => {
	it("applies to visual-only Venue objects but not lamps or crowd areas", () => {
		expect(hasModelScale(fixture("visual_only"))).toBe(true);
		expect(hasModelScale(fixture("dmx"))).toBe(false);
		expect(hasModelScale(fixture("visual_only", null, true))).toBe(false);
	});

	it("reads an object placed before the scale existed at its built size", () => {
		expect(modelScaleOf(fixture("visual_only"))).toBe(1);
		expect(modelScaleOf(fixture("visual_only", null))).toBe(1);
		expect(modelScaleOf(fixture("visual_only", 2.5))).toBe(2.5);
		expect(modelScaleOf(fixture("visual_only", 0))).toBe(1);
		expect(formatModelScale(0.125)).toBe("0.125×");
		expect(formatModelScale(2)).toBe("2×");
	});

	it("stores a typed scale, clears 1 and empty, and refuses outside 0.01 to 100", () => {
		expect(modelScaleChange("2")).toEqual({ model_scale: 2 });
		expect(modelScaleChange("0.5×")).toEqual({ model_scale: 0.5 });
		expect(modelScaleChange("1")).toEqual({ model_scale: null });
		expect(modelScaleChange("")).toEqual({ model_scale: null });
		expect(modelScaleChange("0.01")).toEqual({ model_scale: 0.01 });
		expect(modelScaleChange("100")).toEqual({ model_scale: 100 });
		for (const refused of ["0", "0.001", "101", "-2", "big"])
			expect(modelScaleChange(refused)).toEqual({
				error: "Enter a scale from 0.01 to 100; 1 is the size it was built at.",
			});
	});
});
