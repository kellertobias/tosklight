import { describe, expect, it } from "vitest";
import { stageFixtureDots } from "./stageFixtureDots";

const positions = [
	{
		fixture_id: "root-1",
		physical_fixture_id: "root-1",
		position: { x: 1, y: 2, z: 3 },
	},
	{
		fixture_id: "head-1",
		physical_fixture_id: "root-1",
		position: { x: 1.5, y: 2, z: 3.5 },
	},
	{
		fixture_id: "root-2",
		physical_fixture_id: "root-2",
		position: { x: -1, y: 2, z: 3 },
	},
];

describe("physical fixture projection dots", () => {
	it("draws the authoritative selected head position without substituting its parent's origin", () => {
		expect(stageFixtureDots(["head-1", "missing", "head-1"], positions)).toEqual([
			{ id: "head-1", selected: true, position: positions[1].position },
		]);
	});

	it("draws each physical root once when no selection, without head aliases using the dot limit", () => {
		expect(stageFixtureDots([], positions, 2).map((dot) => dot.id)).toEqual([
			"root-1",
			"root-2",
		]);
		expect(stageFixtureDots([], [])).toEqual([]);
	});

	it("does not invent a coordinate for absent or invalid authoritative positions", () => {
		expect(stageFixtureDots(["missing"], positions)).toEqual([]);
		expect(
			stageFixtureDots([], [
				{ ...positions[0], position: { x: Number.NaN, y: 0, z: 0 } },
			]),
		).toEqual([]);
	});
});
