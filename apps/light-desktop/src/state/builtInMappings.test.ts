import { describe, expect, it } from "vitest";
import {
	builtInForDeskCommand,
	builtIns,
	builtInsForShift,
	shiftedBuiltIns,
} from "./builtInMappings";

describe("Shift-held Built-ins", () => {
	it("uses the exact normal and alternate destinations in the same six positions", () => {
		expect(builtIns.map(([kind, , label]) => [kind, label])).toEqual([
			["stage", "Stage"],
			["fixtures", "Fixtures"],
			["presets", "Presets"],
			["cuelists", "Cue Lists"],
			["dynamics", "Dynamics"],
			["channels", "Channels"],
		]);
		expect(shiftedBuiltIns.map(([kind, , label]) => [kind, label])).toEqual([
			["dmx", "DMX"],
			["media", "Media"],
			["groups", "Groups"],
			["timecode", "Timecode"],
			["macros", "Macro"],
			["scheduler", "Scheduler"],
		]);
	});

	it("restores the normal destinations when Shift is released", () => {
		expect(builtInsForShift(true)).toBe(shiftedBuiltIns);
		expect(builtInsForShift(false)).toBe(builtIns);
	});

	it.each([
		["stage", "stage", "dmx"],
		["fixtures", "fixtures", "media"],
		["presets", "presets", "groups"],
		["cues", "cuelists", "timecode"],
		["dynamics", "dynamics", "macros"],
		["channels", "channels", "scheduler"],
	] as const)("maps the %s attached desk command through the same normal and Shift destinations", (command, normal, shifted) => {
		expect(builtInForDeskCommand(command, false)).toBe(normal);
		expect(builtInForDeskCommand(command, true)).toBe(shifted);
	});
});
