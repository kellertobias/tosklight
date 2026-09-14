import { describe, expect, it } from "vitest";
import { type DrawnPart, hideCoveredEdges } from "./hiddenLines";
import type { PlanPoint } from "./projection";

const square = (x: number, y: number, size: number): PlanPoint[] => [
	[x, y],
	[x + size, y],
	[x + size, y + size],
	[x, y + size],
];
const solid = (points: PlanPoint[]): DrawnPart => ({
	kind: "solid",
	edges: [points],
	area: [points],
});
const length = (lines: ReturnType<typeof hideCoveredEdges>["lines"]) =>
	lines.reduce(
		(total, { points: [[ax, ay], [bx, by]] }) => total + Math.hypot(bx - ax, by - ay),
		0,
	);

describe("hidden-line removal", () => {
	it("keeps a shape nothing covers as a whole outline", () => {
		const result = hideCoveredEdges([solid(square(0, 0, 10)), solid(square(20, 0, 10))]);
		expect(result.outlines).toHaveLength(2);
		expect(result.lines).toHaveLength(0);
	});

	it("hides the edges of a shape where a later shape covers them, and never the other way", () => {
		const result = hideCoveredEdges([solid(square(0, 0, 10)), solid(square(5, 5, 10))]);
		// The front square stays whole; the back square keeps only what shows: 10 + 5 + 5 + 10.
		expect(result.outlines).toEqual([square(5, 5, 10)]);
		expect(length(result.lines)).toBeCloseTo(30, 3);
		const hiddenPoint = result.lines.some(({ points }) =>
			points.some(([x, y]) => x > 5 + 1e-6 && y > 5 + 1e-6),
		);
		expect(hiddenPoint).toBe(false);
	});

	it("leaves an edge lying on a later shape's boundary to that shape", () => {
		// A bar ending exactly against a deck: its end edge coincides with the deck's edge.
		const bar: PlanPoint[] = square(0, 0, 10);
		const deck: PlanPoint[] = [
			[-5, 10],
			[15, 10],
			[15, 12],
			[-5, 12],
		];
		const result = hideCoveredEdges([solid(bar), solid(deck)]);
		expect(result.outlines).toEqual([deck]);
		expect(length(result.lines)).toBeCloseTo(30, 3);
		expect(
			result.lines.some(({ points }) => points.every(([, y]) => Math.abs(y - 10) < 1e-6)),
		).toBe(false);
	});

	it("cuts an open line where a later shape covers it, and lets a hollow ring show through", () => {
		const ring: DrawnPart = {
			kind: "solid",
			edges: [square(0, 0, 30), square(10, 10, 10)],
			area: [
				[
					[0, 0],
					[30, 0],
					[30, 10],
					[0, 10],
				],
				[
					[0, 20],
					[30, 20],
					[30, 30],
					[0, 30],
				],
				[
					[0, 10],
					[10, 10],
					[10, 20],
					[0, 20],
				],
				[
					[20, 10],
					[30, 10],
					[30, 20],
					[20, 20],
				],
			],
		};
		const result = hideCoveredEdges([
			{ kind: "line", line: { points: [[-10, 15], [40, 15]] } },
			ring,
		]);
		// Left of the ring, through its opening, right of it.
		expect(length(result.lines)).toBeCloseTo(10 + 10 + 10, 3);
		expect(result.outlines).toHaveLength(2);
	});
});
