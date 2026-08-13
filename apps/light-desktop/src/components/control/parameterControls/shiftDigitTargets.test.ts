import { describe, expect, it } from "vitest";
import { parameterTargetForShiftDigit } from "./useParameterController";

describe("shifted numeric encoder targets", () => {
	it("maps all ten documented keys", () => {
		expect(Array.from({ length: 10 }, (_, key) => parameterTargetForShiftDigit(String(key)))).toEqual([
			{ type: "all" },
			{ type: "family", family: "Intensity" },
			{ type: "family", family: "Color" },
			{ type: "family", family: "Position" },
			{ type: "family", family: "Beam" },
			{ type: "dynamics" },
			{ type: "family", family: "Shapers" },
			{ type: "family", family: "Focus" },
			{ type: "family", family: "Control" },
			{ type: "family", family: "Media" },
		]);
	});
});
