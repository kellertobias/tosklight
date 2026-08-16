import { describe, expect, it } from "vitest";
import {
	applyGestureCommand,
	resolveCommandKeyGesture,
} from "./commandKeyGesture";

describe("command-key gesture map", () => {
	it.each([
		["CUE", false, "CUELIST"],
		["PLAYBACK", false, "VPBK"],
		["GRP", false, "DEGROUP"],
		["GRP", true, "DMX"],
		["DIV", true, "LOAD"],
		["CLR", true, "UNFREEZE"],
		["2", true, "COLOR PRESET"],
	] as const)("maps double %s shifted=%s to %s", (key, shifted, text) => {
		const intent = resolveCommandKeyGesture(key, { kind: "double", shifted });
		expect(intent).toMatchObject({ type: "command", text });
	});

	it.each([
		["0", "ALL"],
		["1", "INTENSITY"],
		["2", "COLOR"],
		["3", "POSITION"],
		["4", "BEAM"],
		["5", "DYNAMICS"],
		["6", "SHAPERS"],
		["7", "FOCUS"],
		["8", "CONTROL"],
		["9", "MEDIA"],
		["AT", "FixAT"],
		["GRP", "FIXTURE"],
		["CUE", "TIMECODE"],
		["PLAYBACK", "MACRO"],
		["SET", "ASSIGN"],
		["TIME", "SPD GRP"],
		["DIV", "GO TO"],
		["OFF", "RELEASE"],
		["MOV", "COPY"],
		["REC", "UPDATE"],
		["CLR", "FREEZE"],
	] as const)("maps shifted %s to %s", (key, text) => {
		expect(
			resolveCommandKeyGesture(key, { kind: "regular", shifted: true }),
		).toMatchObject({ type: "command", text });
	});

	it("replaces the first press when a double gesture resolves", () => {
		const intent = resolveCommandKeyGesture("CUE", {
			kind: "double",
			shifted: false,
		});
		if (!intent || intent.type !== "command") throw new Error("missing intent");
		expect(applyGestureCommand("RECORD CUE", false, intent)).toBe(
			"RECORD CUELIST",
		);
	});

	it.each([
		["GRP", false, "inspect-groups"],
		["GRP", true, "inspect-fixtures"],
		["REC", false, "record-options"],
		["REC", true, "update-options"],
		["PRE", false, "inspect-preload"],
	] as const)("maps hold %s shifted=%s to %s", (key, shifted, action) => {
		expect(
			resolveCommandKeyGesture(key, { kind: "hold", shifted }),
		).toEqual({ type: "action", action });
	});
});
