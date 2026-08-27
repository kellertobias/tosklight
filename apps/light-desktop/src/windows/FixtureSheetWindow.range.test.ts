import { describe, expect, it } from "vitest";
import { fixtureRange } from "./FixtureSheetWindow";

const rows = (...ids: string[]) =>
	ids.map((parentFixtureId) => ({ parentFixtureId }));

describe("Shift-selecting a range in the fixture sheet", () => {
	it("takes every fixture between the two the operator clicked", () => {
		expect(fixtureRange(rows("a", "b", "c", "d"), "a", "c")).toEqual([
			"a",
			"b",
			"c",
		]);
	});

	it("reads the same range clicked from either end", () => {
		expect(fixtureRange(rows("a", "b", "c", "d"), "d", "b")).toEqual([
			"b",
			"c",
			"d",
		]);
	});

	it("follows the order the sheet is showing, not the fixture numbers", () => {
		// Sorted or filtered, the run an operator sees between two rows is what they mean, even
		// when those rows are not next to each other by number.
		expect(fixtureRange(rows("c", "a", "d", "b"), "c", "d")).toEqual([
			"c",
			"a",
			"d",
		]);
	});

	it("counts a fixture once however many rows it spans", () => {
		// A fixture with several attribute rows is still one fixture to select.
		expect(fixtureRange(rows("a", "a", "b", "b", "c"), "a", "b")).toEqual([
			"a",
			"b",
		]);
	});

	it("selects the one fixture when both ends are the same", () => {
		expect(fixtureRange(rows("a", "b", "c"), "b", "b")).toEqual(["b"]);
	});

	it("gives up rather than guessing when the anchor has been filtered away", () => {
		expect(fixtureRange(rows("b", "c"), "a", "c")).toBeNull();
	});
});
