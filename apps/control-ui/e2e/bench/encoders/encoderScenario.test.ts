import { describe, expect, expectTypeOf, it } from "vitest";
import type {
	AbsoluteEncoderPort,
	NormalizedEncoderPort,
	RelativeEncoderPort,
} from "./encoderScenario";

describe("encoder route capabilities", () => {
	it("keeps absolute value entry and relative OSC detents distinct in the type contract", () => {
		expectTypeOf<AbsoluteEncoderPort>().toHaveProperty("set");
		expectTypeOf<AbsoluteEncoderPort>().not.toHaveProperty("add");
		expectTypeOf<RelativeEncoderPort>().toHaveProperty("add");
		expectTypeOf<RelativeEncoderPort>().toHaveProperty("subtract");
		expectTypeOf<RelativeEncoderPort>().not.toHaveProperty("set");
		expectTypeOf<NormalizedEncoderPort>().toHaveProperty("set");
		expectTypeOf<NormalizedEncoderPort>().toHaveProperty("add");
		expect(true).toBe(true);
	});
});
