import { describe, expect, it } from "vitest";
import { programmerValuesWriteUnavailableReason } from "./useProgrammerValuesMutationQueue";

describe("programmerValuesWriteUnavailableReason", () => {
	it.each([
		[false, "loading", false, false, "Channel controls are inactive"],
		[true, "loading", false, false, "Programmer mode is loading"],
		[true, "normal", false, false, "Programmer values are loading"],
		[true, "preload", false, false, "Preload values are loading"],
		[true, "normal", true, false, "Programmer control is unavailable"],
		[true, "preload", true, false, "Preload control is unavailable"],
		[true, "normal", true, true, null],
		[true, "preload", true, true, null],
	] as const)("reports enabled=%s authority=%s ready=%s actions=%s", (enabled, authority, ready, actionsAvailable, expected) => {
		expect(
			programmerValuesWriteUnavailableReason(
				enabled,
				{ authority, ready },
				actionsAvailable,
			),
		).toBe(expected);
	});
});
