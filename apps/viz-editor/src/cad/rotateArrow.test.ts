import { describe, expect, it } from "vitest";
import { rotateArrowGeometry } from "./CadApp";

/** The numbers of an SVG path in order. */
const numbers = (path: string) => [...path.matchAll(/-?\d+(?:\.\d+)?/gu)].map(Number);
const CENTRE = 52;

/** Degrees counterclockwise from the right, as the operator reads the circle. */
const angleOf = (x: number, y: number) =>
	(Math.atan2(CENTRE - y, x - CENTRE) * 180) / Math.PI;

describe("the top-down view's rotate arrows", () => {
	it("run short arcs at the circle's top right, heads at their outer ends", () => {
		const clockwise = rotateArrowGeometry(40, 20);
		const counterclockwise = rotateArrowGeometry(50, 70);

		const [cx1, cy1, , , , , , cx2, cy2] = numbers(clockwise.arc);
		expect(angleOf(cx1, cy1)).toBeCloseTo(40, 0);
		expect(angleOf(cx2, cy2)).toBeCloseTo(20, 0);
		const [ax1, ay1, , , , , , ax2, ay2] = numbers(counterclockwise.arc);
		expect(angleOf(ax1, ay1)).toBeCloseTo(50, 0);
		expect(angleOf(ax2, ay2)).toBeCloseTo(70, 0);

		// Each head's tip is the arc's end, the end further from the other arrow.
		expect(numbers(clockwise.head).slice(2, 4)).toEqual([cx2, cy2]);
		expect(numbers(counterclockwise.head).slice(2, 4)).toEqual([ax2, ay2]);

		// Both lie in the top right quarter around the circle, and the buttons do not overlap.
		for (const { box } of [clockwise, counterclockwise]) {
			expect(box.x).toBeGreaterThanOrEqual(CENTRE);
			expect(box.y + box.height).toBeLessThanOrEqual(CENTRE);
		}
		expect(counterclockwise.box.y + counterclockwise.box.height).toBeLessThanOrEqual(
			clockwise.box.y + clockwise.box.height,
		);
	});

	it("sweeps clockwise on screen for the clockwise arrow and back for the other", () => {
		expect(rotateArrowGeometry(40, 20).arc).toMatch(/ 0 0 1 /u);
		expect(rotateArrowGeometry(50, 70).arc).toMatch(/ 0 0 0 /u);
	});
});
