import { describe, expect, it } from "vitest";
import {
	numericPadLayout,
	oscProgrammerActionForKey,
	softwareKeyLabel,
} from "./programmerKeypad";

describe("programmer keypad contract", () => {
	it("keeps the shared physical layout stable", () => {
		expect(numericPadLayout).toHaveLength(34);
		expect(numericPadLayout.find(({ key }) => key === "DEL")).toMatchObject({
			section: "commands",
			column: 1,
			row: 2,
		});
		expect(numericPadLayout.find(({ key }) => key === "ENT")).toMatchObject({
			section: "numbers",
			column: 7,
			row: 5,
		});
	});

	it("maps software keys to the established OSC vocabulary and labels", () => {
		expect(oscProgrammerActionForKey("7")).toBe("digit-7");
		expect(oscProgrammerActionForKey("TRU")).toBe("thru");
		expect(oscProgrammerActionForKey("BACKSPACE")).toBe("backspace");
		expect(oscProgrammerActionForKey("CLR")).toBe("clear");
		expect(oscProgrammerActionForKey("UND")).toBe("undo");
		expect(oscProgrammerActionForKey("REC")).toBe("record");
		expect(oscProgrammerActionForKey("PRE")).toBe("preload");
		expect(softwareKeyLabel("BACKSPACE")).toBe("←");
		expect(softwareKeyLabel("SELECT")).toBe("SELECT");
		expect(oscProgrammerActionForKey("PLAYBACK")).toBe("playback");
		expect(oscProgrammerActionForKey("OFF")).toBe("off");
		expect(softwareKeyLabel("PAGE_UP")).toBe("PAGE ▲");
	});
});
