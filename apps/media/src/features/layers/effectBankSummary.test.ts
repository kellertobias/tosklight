import { describe, expect, it } from "vitest";
import { effectBankSummary } from "./layerDmxSections";

describe("live canonical effect bank summary", () => {
	it("names the active Kaleidoscope bank independently of legacy slots", () => {
		expect(
			effectBankSummary(
				[{ index: 0, select: 9, strength: 1, parameters: [0, 0, 0, 0] }],
				[{ slot: 9, name: "Kaleidoscope" }],
			),
		).toBe("Bank 1 · Kaleidoscope");
	});
	it("preserves bank order and selected slot identity before library metadata arrives", () => {
		expect(
			effectBankSummary(
				[
					{ index: 0, select: 9, strength: 1, parameters: [0, 0, 0, 0] },
					{ index: 1, select: 3, strength: 0.5, parameters: [0, 0, 0, 0] },
				],
				[{ slot: 3, name: "Blur" }],
			),
		).toBe("Bank 1 · Slot 9 · Bank 2 · Blur");
	});
	it("reports None for off or zero-strength banks", () => {
		expect(
			effectBankSummary(
				[
					{ index: 0, select: 0, strength: 1, parameters: [0, 0, 0, 0] },
					{ index: 1, select: 9, strength: 0, parameters: [0, 0, 0, 0] },
				],
				[{ slot: 9, name: "Kaleidoscope" }],
			),
		).toBe("None");
	});
});
