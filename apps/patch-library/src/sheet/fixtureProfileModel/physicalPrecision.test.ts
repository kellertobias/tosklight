import { describe, expect, it } from "vitest";
import {
	hasAtMostDecimals,
	opticsPercentMessage,
	percentFraction,
	percentText,
	precisionMessage,
} from "./physicalPrecision";

describe("fixture-profile physical precision", () => {
	it("accepts figures at their precision, including 32-bit values the desk widened", () => {
		expect(hasAtMostDecimals(420, 0)).toBe(true);
		expect(hasAtMostDecimals(24.55, 2)).toBe(true);
		expect(hasAtMostDecimals(Math.fround(1.98), 2)).toBe(true);
		expect(hasAtMostDecimals(420.5, 0)).toBe(false);
		expect(hasAtMostDecimals(1.975, 2)).toBe(false);
		expect(precisionMessage("Weight", "kg", 2, Math.fround(1.98))).toBeNull();
	});

	it("names the unit a whole-number figure is kept in", () => {
		expect(precisionMessage("Depth", "mm", 0, 0.5)).toBe(
			"Depth must be a whole number of millimetres",
		);
		expect(precisionMessage("Power consumption", "W", 0, 1.5)).toBe(
			"Power consumption must be a whole number of watts",
		);
		expect(precisionMessage("Depth", "mm", 0, null)).toBeNull();
	});

	it("shows sharpness and uniformity with exactly one decimal place", () => {
		expect(percentText(0.85)).toBe("85.0");
		expect(percentText(Math.fround(0.289))).toBe("28.9");
		expect(percentText(1)).toBe("100.0");
		expect(percentText(null)).toBe("");
		// An off-precision stored figure is shown as it is, and named.
		expect(percentText(0.8555)).toBe("85.55");
		expect(opticsPercentMessage("Uniformity", 0.8555)).toBe(
			"Uniformity must be a percentage with one decimal place",
		);
		expect(opticsPercentMessage("Uniformity", Math.fround(0.625))).toBeNull();
	});

	it("reads a typed percentage into the stored fraction within 0 to 100", () => {
		expect(percentFraction("28.9")).toBe(0.289);
		expect(percentFraction("150")).toBe(1);
		expect(percentFraction("-3")).toBe(0);
		expect(percentFraction("")).toBeNull();
		expect(percentFraction("85.55")).toBeCloseTo(0.8555, 10);
	});
});
