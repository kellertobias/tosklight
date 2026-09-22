import { describe, expect, it } from "vitest";
import { parseMetreList, stageGridPlacements, trussRowPlacements } from "./bulkPlacement";

describe("a field of stage elements", () => {
	const deck = { width: 2, depth: 1 };

	it("butts the elements edge to edge, with no gap anywhere in the field", () => {
		const grid = stageGridPlacements({ columns: 3, rows: 2, turned: false, footprint: deck });
		expect(grid).toHaveLength(6);
		expect(grid.map((each) => [each.position.x, each.position.y])).toEqual([
			[0, 0],
			[2000, 0],
			[4000, 0],
			[0, 1000],
			[2000, 1000],
			[4000, 1000],
		]);
	});

	it("puts its first element exactly where one pressed on its own would land", () => {
		const [first] = stageGridPlacements({ columns: 4, rows: 4, turned: false, footprint: deck });
		expect(first.position).toEqual({ x: 0, y: 0, z: 0 });
	});

	it("turns the step with the elements, so a deck on its side steps its own short side across", () => {
		const grid = stageGridPlacements({ columns: 2, rows: 2, turned: true, footprint: deck });
		expect(grid.map((each) => [each.position.x, each.position.y])).toEqual([
			[0, 0],
			[1000, 0],
			[0, 2000],
			[1000, 2000],
		]);
		for (const each of grid) expect(each.rotation.z).toBe(90);
	});

	it("places at least one element however few are asked for", () => {
		expect(stageGridPlacements({ columns: 0, rows: -2, turned: false, footprint: deck })).toHaveLength(1);
	});
});

describe("rows of truss", () => {
	it("flies the run again at every height, over every line", () => {
		const rows = trussRowPlacements({ heights: [5, 7], positions: [0, 4], turned: false });
		expect(rows.map((each) => [each.position.x, each.position.y, each.position.z])).toEqual([
			[0, 0, 5000],
			[0, 4000, 5000],
			[0, 0, 7000],
			[0, 4000, 7000],
		]);
		for (const each of rows) expect(each.rotation.z).toBe(0);
	});

	it("crosses the heights with X instead when the runs are turned a quarter turn", () => {
		const rows = trussRowPlacements({ heights: [6], positions: [-3, 3], turned: true });
		expect(rows.map((each) => [each.position.x, each.position.y, each.position.z])).toEqual([
			[-3000, 0, 6000],
			[3000, 0, 6000],
		]);
		for (const each of rows) expect(each.rotation.z).toBe(90);
	});

	it("places nothing when either list is empty", () => {
		expect(trussRowPlacements({ heights: [], positions: [1], turned: false })).toEqual([]);
		expect(trussRowPlacements({ heights: [5], positions: [], turned: false })).toEqual([]);
	});
});

describe("a typed list of metres", () => {
	it("reads values separated by spaces, with a comma as the decimal point", () => {
		expect(parseMetreList("4 6 8")).toEqual([4, 6, 8]);
		expect(parseMetreList("4,5 6")).toEqual([4.5, 6]);
	});

	it("reads a list written with commas between the values", () => {
		expect(parseMetreList("4, 6, 8")).toEqual([4, 6, 8]);
	});

	it("counts out an evenly spaced run rather than making the operator type it", () => {
		expect(parseMetreList("4 THRU 8 BY 2")).toEqual([4, 6, 8]);
		expect(parseMetreList("4 THRU 6")).toEqual([4, 5, 6]);
		expect(parseMetreList("6 THRU 4")).toEqual([6, 5, 4]);
	});

	it("is empty for an empty field, and refuses anything it cannot read", () => {
		expect(parseMetreList("   ")).toEqual([]);
		expect(parseMetreList("4,5,6")).toBeNull();
		expect(parseMetreList("high")).toBeNull();
		expect(parseMetreList("4 THRU 8 BY 0")).toBeNull();
	});
});
