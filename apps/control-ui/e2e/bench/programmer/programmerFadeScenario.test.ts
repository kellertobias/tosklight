import { describe, expect, expectTypeOf, it } from "vitest";
import type { ClockDuration } from "../core/clockScenario";
import type { ProgrammerFadeSetPort } from "./programmerFadeScenario";

describe("Programmer Fade scenario contract", () => {
	it("keeps every set route on the shared typed duration contract", () => {
		expectTypeOf<ProgrammerFadeSetPort["set"]>().parameter(0).toEqualTypeOf<
			ClockDuration
		>();
	});

	it("requires a duration unit", () => {
		const accepts = (duration: ClockDuration) => duration;
		expect(accepts("1250ms")).toBe("1250ms");
		// @ts-expect-error bare numbers do not identify a duration unit
		expect(accepts(1250)).toBe(1250);
	});
});
