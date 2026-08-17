import { describe, expect, it } from "vitest";
import { encoderDelta } from "./useHardwareParameterEncoders";

describe("hardware parameter encoder steps", () => {
	it("steps byte-addressed media sources one address at a time", () => {
		for (const attribute of [
			"media.folder",
			"media.file",
			"media.mask.folder",
			"media.mask.file",
		]) {
			expect(encoderDelta(attribute, "up")).toBe(1 / 255);
			expect(encoderDelta(attribute, "down")).toBe(-1 / 255);
		}
	});

	it("preserves fine and coarse normalized steps for continuous attributes", () => {
		expect(encoderDelta("intensity", "up")).toBe(0.01);
		expect(encoderDelta("intensity", "right")).toBe(0.1);
	});
});
