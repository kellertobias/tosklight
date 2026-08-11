import { describe, expect, it } from "vitest";
import { resolveMacroPoolGesture } from "./poolGesture";

describe("Macro pool gestures", () => {
	it("runs an occupied Macro on an ordinary primary tap", () => {
		expect(resolveMacroPoolGesture(true, false, false)).toBe("run");
	});

	it("edits an occupied Macro for SET-click and secondary click", () => {
		expect(resolveMacroPoolGesture(true, true, false)).toBe("edit");
		expect(resolveMacroPoolGesture(true, false, true)).toBe("edit");
	});

	it("opens creation for an empty pool slot with either button", () => {
		expect(resolveMacroPoolGesture(false, false, false)).toBe("create");
		expect(resolveMacroPoolGesture(false, false, true)).toBe("create");
	});
});
