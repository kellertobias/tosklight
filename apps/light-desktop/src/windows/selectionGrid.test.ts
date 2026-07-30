import { describe, expect, it } from "vitest";
import {
	columnsFirst,
	rowsFirst,
	selectionGridCells,
} from "./selectionGrid";

const positions3d = {
	a: { x: 0, y: 0, z: 1, rotationX: 0, rotationY: 0, rotationZ: 0 },
	b: { x: 2, y: 0, z: 1, rotationX: 0, rotationY: 0, rotationZ: 0 },
	c: { x: 0, y: 0, z: 0, rotationX: 0, rotationY: 0, rotationZ: 0 },
	d: { x: 2, y: 0, z: 0, rotationX: 0, rotationY: 0, rotationZ: 0 },
};

describe("selectionGridCells", () => {
	it("preserves Stage holes and places zero-based rows from top to bottom", () => {
		expect(
			selectionGridCells(
				["a", "b", "c"],
				{ method: "stage2d" },
				{
					positions2d: {
						a: { x: 0, y: 0 },
						b: { x: 2, y: 0 },
						c: { x: 0, y: 2 },
					},
					positions3d: {},
				},
			),
		).toEqual([
			{ fixtureId: "a", row: 0, column: 0 },
			{ fixtureId: "b", row: 0, column: 1 },
			{ fixtureId: "c", row: 1, column: 0 },
		]);
	});

	it("expands exact ties by stable fixture identity and retains missing fixtures", () => {
		const cells = selectionGridCells(
			["z", "a", "missing"],
			{ method: "front_to_back" },
			{
				positions2d: {},
				positions3d: { z: positions3d.a, a: positions3d.a },
			},
		);
		expect(cells).toEqual([
			{ fixtureId: "a", row: 0, column: 0 },
			{ fixtureId: "z", row: 0, column: 1 },
			{ fixtureId: "missing", row: 1, column: 0 },
		]);
	});

	it("supports all row-first and column-first sparse traversals", () => {
		const cells = selectionGridCells(
			["a", "b", "c", "d"],
			{ method: "front_to_back" },
			{ positions2d: {}, positions3d },
		);
		expect(rowsFirst(cells, "top_left")).toEqual(["a", "b", "c", "d"]);
		expect(rowsFirst(cells, "top_right")).toEqual(["b", "a", "d", "c"]);
		expect(rowsFirst(cells, "bottom_left")).toEqual(["c", "d", "a", "b"]);
		expect(rowsFirst(cells, "bottom_right")).toEqual(["d", "c", "b", "a"]);
		expect(columnsFirst(cells, "top_left")).toEqual(["a", "c", "b", "d"]);
		expect(columnsFirst(cells, "bottom_left")).toEqual(["c", "a", "d", "b"]);
		expect(columnsFirst(cells, "top_right")).toEqual(["b", "d", "a", "c"]);
		expect(columnsFirst(cells, "bottom_right")).toEqual(["d", "b", "c", "a"]);
	});
});
