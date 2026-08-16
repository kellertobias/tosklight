import { describe, expect, it } from "vitest";
import { dynamicPoolCommand } from "./DynamicsWindow";

describe("Dynamic pool command routing", () => {
	it("uses the retained Group scope and leaves clearing to authoritative execution", () => {
		expect(dynamicPoolCommand("GROUP 18", false, 29)).toBe(
			"GROUP 18 DYNAMIC 29",
		);
	});

	it("treats OFF followed by a Dynamic as explicit Dynamic off", () => {
		expect(dynamicPoolCommand("OFF", false, 29)).toBe("DYNAMIC 29 OFF");
		expect(dynamicPoolCommand("GROUP 18 OFF", false, 29)).toBe(
			"GROUP 18 DYNAMIC 29 OFF",
		);
	});

	it("does not include pristine Fixture or Group prompts", () => {
		expect(dynamicPoolCommand("FIXTURE", true, 29)).toBe("DYNAMIC 29");
		expect(dynamicPoolCommand("GROUP", true, 29)).toBe("DYNAMIC 29");
	});
});
