import { describe, expect, it } from "vitest";
import { parseMibInput } from "./editSave";

describe("combined MIB value parsing", () => {
	it("preserves Off as a distinct disabled state", () => {
		expect(parseMibInput("  oFf ")).toEqual({
			changes: {
				move_in_black_enabled: false,
				move_in_black_delay_millis: 0,
			},
		});
	});

	it.each([
		["0", 0],
		["0.0005", 1],
		["0.5", 500],
		["12", 12_000],
	] as const)("parses an enabled %s second delay", (value, milliseconds) => {
		expect(parseMibInput(value)).toEqual({
			changes: {
				move_in_black_enabled: true,
				move_in_black_delay_millis: milliseconds,
			},
		});
	});

	it.each([
		"",
		"-0.1",
		"-0",
		"+1",
		"0x10",
		"1e3",
		"NaN",
		"Infinity",
	])("rejects invalid input %s", (value) => {
		expect(parseMibInput(value)).toEqual({
			error: "Enter Off or a finite non-negative delay in seconds.",
		});
	});

	it("rejects millisecond overflow", () => {
		expect(parseMibInput(String(Number.MAX_SAFE_INTEGER))).toEqual({
			error: "The MIB delay is too large to store safely.",
		});
	});
});
