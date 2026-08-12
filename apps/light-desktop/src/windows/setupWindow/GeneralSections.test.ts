import { describe, expect, it } from "vitest";
import { parseAudioBindingMap } from "./GeneralSections";

describe("desk-local audio binding maps", () => {
	it("maps logical show names to local targets", () => {
		expect(
			parseAudioBindingMap("house = /Volumes/Show Audio\nmain = $system_default"),
		).toEqual({ house: "/Volumes/Show Audio", main: "$system_default" });
	});

	it("rejects duplicate names and incomplete mappings", () => {
		expect(parseAudioBindingMap("main = first\nmain = second")).toBeNull();
		expect(parseAudioBindingMap("main = ")).toBeNull();
	});
});
