import { describe, expect, it } from "vitest";
import {
	builtInForDeskCommand,
	builtIns,
	builtInsForShift,
	shiftedBuiltIns,
} from "./builtInMappings";

describe("Shift-held Built-ins", () => {
	it("replaces the five alternate destinations in their normal positions", () => {
		expect(builtIns.map(([kind]) => kind)).toEqual([
			"stage",
			"fixtures",
			"presets",
			"cuelists",
			"dynamics",
			"channels",
		]);
		expect(shiftedBuiltIns.map(([kind]) => kind)).toEqual([
			"media",
			"groups",
			"presets",
			"timecode",
			"macros",
			"dmx",
		]);
	});

	it("restores the normal destinations when Shift is released", () => {
		expect(builtInsForShift(true)).toBe(shiftedBuiltIns);
		expect(builtInsForShift(false)).toBe(builtIns);
	});

	it.each([
		["stage", "stage", "media"],
		["fixtures", "fixtures", "groups"],
		["presets", "presets", "presets"],
		["cues", "cuelists", "timecode"],
		["dynamics", "dynamics", "macros"],
		["channels", "channels", "dmx"],
	] as const)("maps the %s attached desk command through the same normal and Shift destinations", (command, normal, shifted) => {
		expect(builtInForDeskCommand(command, false)).toBe(normal);
		expect(builtInForDeskCommand(command, true)).toBe(shifted);
	});
});
