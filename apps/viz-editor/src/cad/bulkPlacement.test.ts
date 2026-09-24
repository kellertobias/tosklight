import { describe, expect, it } from "vitest";
import { stageGridPlacements, trussRunPlacements, trussRunSectionLength } from "./bulkPlacement";

describe("a grid of stage elements", () => {
	const deck = { width: 2, depth: 1 };

	it("butts the elements edge to edge, with no gap anywhere, centred on the grid's centre", () => {
		const grid = stageGridPlacements({ columns: 3, rows: 2, turned: false, footprint: deck });
		expect(grid).toHaveLength(6);
		expect(grid.map((each) => [each.position.x, each.position.y])).toEqual([
			[-2000, -500],
			[0, -500],
			[2000, -500],
			[-2000, 500],
			[0, 500],
			[2000, 500],
		]);
		// Neighbours are exactly one footprint apart: touching, never overlapping.
		for (let column = 1; column < 3; column += 1)
			expect(grid[column].position.x - grid[column - 1].position.x).toBe(2000);
		expect(grid[3].position.y - grid[0].position.y).toBe(1000);
	});

	it("stands the whole grid around the centre asked for, at its height", () => {
		const grid = stageGridPlacements({
			columns: 2,
			rows: 2,
			turned: false,
			footprint: deck,
			centre: { x: 4, y: -3, z: 0.6 },
		});
		const xs = grid.map((each) => each.position.x);
		const ys = grid.map((each) => each.position.y);
		expect((Math.min(...xs) + Math.max(...xs)) / 2).toBe(4000);
		expect((Math.min(...ys) + Math.max(...ys)) / 2).toBe(-3000);
		for (const each of grid) expect(each.position.z).toBe(600);
	});

	it("turns the step with the elements, so a deck on its side steps its own short side across", () => {
		const grid = stageGridPlacements({ columns: 2, rows: 2, turned: true, footprint: deck });
		expect(grid.map((each) => [each.position.x, each.position.y])).toEqual([
			[-500, -1000],
			[500, -1000],
			[-500, 1000],
			[500, 1000],
		]);
		for (const each of grid) expect(each.rotation.z).toBe(90);
	});

	it("places at least one element however few are asked for", () => {
		expect(stageGridPlacements({ columns: 0, rows: -2, turned: false, footprint: deck })).toHaveLength(1);
	});
});

describe("a run of truss", () => {
	it("spaces the sections evenly from the first point to the last, filling the run end to end", () => {
		const run = { first: { x: 0, y: 0, z: 6 }, last: { x: 8, y: 0, z: 6 }, count: 4 };
		const sections = trussRunPlacements(run);
		expect(sections.map((each) => each.position.x)).toEqual([1000, 3000, 5000, 7000]);
		expect(trussRunSectionLength(run)).toBe(2);
		for (const each of sections) expect(each.rotation).toEqual({ x: 0, y: 0, z: 0 });
	});

	it("heads the sections along the run on the plan, turning about Z only", () => {
		const diagonal = trussRunPlacements({ first: { x: 0, y: 0, z: 5 }, last: { x: 3, y: 3, z: 5 }, count: 2 });
		for (const each of diagonal) expect(each.rotation).toEqual({ x: 0, y: 0, z: 45 });
		const downstage = trussRunPlacements({ first: { x: 0, y: 4, z: 5 }, last: { x: 0, y: -4, z: 5 }, count: 1 });
		expect(downstage[0].rotation).toEqual({ x: 0, y: 0, z: -90 });
		expect(downstage[0].position).toEqual({ x: 0, y: 0, z: 5000 });
	});

	it("raises each section to the height of its own place along a run whose ends differ in height", () => {
		const sections = trussRunPlacements({ first: { x: 0, y: 0, z: 4 }, last: { x: 10, y: 0, z: 6 }, count: 5 });
		expect(sections.map((each) => each.position.z)).toEqual([4200, 4600, 5000, 5400, 5800]);
		// Still no pitch: a raised run is stepped, never tilted.
		for (const each of sections) expect(each.rotation.y).toBe(0);
	});

	it("places at least one section however few are asked for", () => {
		expect(trussRunPlacements({ first: { x: 0, y: 0, z: 0 }, last: { x: 1, y: 0, z: 0 }, count: 0 })).toHaveLength(1);
	});
});
