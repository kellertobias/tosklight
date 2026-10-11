import { describe, expect, it } from "vitest";
import { parseSoloMembers } from "./PlaybackSoloSettings";

describe("general Solo Region membership", () => {
	it("preserves ordered members across exact pages and sparse positions", () => {
		expect(parseSoloMembers("1.15, 2.201, 99.1000")).toEqual([
			{ page: 1, position: 15 },
			{ page: 2, position: 201 },
			{ page: 99, position: 1000 },
		]);
	});
	it.each([
		"1.1",
		"1.1, 1.1",
		"0.1, 2.1",
		"100.1, 1.1",
		"1.1001, 2.1",
		"1, 2",
	])("rejects ambiguous or invalid members %s", (value) => {
		expect(() => parseSoloMembers(value)).toThrow();
	});
});
