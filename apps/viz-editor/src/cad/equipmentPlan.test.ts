import type { FixtureProfileScenery } from "@tosklight/patch";
import { describe, expect, it } from "vitest";
import { discoBallPlan, flightRackPlan, lineArrayPlan, paSpeakerPlan, rackHeight } from "./equipmentPlan";
import { sizeMeasures } from "./sceneryAxes";

const equipment = (
	kind: FixtureProfileScenery["kind"],
	size: [number, number, number],
	minimum: [number, number, number],
	maximum: [number, number, number],
	depth = false,
): FixtureProfileScenery => {
	const vector = ([x, y, z]: [number, number, number]) => ({ x, y, z });
	return {
		kind,
		chords: 0,
		default_size_metres: vector(size),
		adjustable: { width: false, height: true, depth },
		minimum_size_metres: vector(minimum),
		maximum_size_metres: vector(maximum),
	};
};
const rack = equipment("flight_rack", [0.6, 0.3867, 0.6], [0.6, 0.16445, 0.4], [0.6, 1.1868, 1], true);
const pa = equipment("pa_top", [0.35, 0.6, 0.4], [0.35, 0.6, 0.4], [0.35, 2.6, 0.4]);
const array = equipment("line_array", [1, 2.1, 0.6], [1, 0.35, 0.6], [1, 6.1, 0.6]);
const ball = {
	...equipment("mirror_ball", [0.5, 0.75, 0.5], [0.2, 0.2, 0.2], [1.5, 4.5, 1.5]),
	adjustable: { width: true, height: true, depth: false },
};

describe("equipment measured the way it is bought", () => {
	it("offers a rack by its units and its depth, and stores the height those units need", () => {
		const [units, depth] = sizeMeasures(rack);
		expect([units.label, units.unit, units.min, units.max]).toEqual(["Units", "U", 1, 24]);
		expect(depth.label).toBe("Depth");
		const size = { x: 0.6, y: 0.3867, z: 0.6 };
		expect(units.read(size)).toBe(6);
		expect(units.write(size, 12).y).toBeCloseTo(rackHeight(12) / 1000, 6);
		expect(depth.write(size, 0.8).z).toBe(0.8);
	});

	it("offers a line array by its elements", () => {
		const [elements] = sizeMeasures(array);
		expect([elements.label, elements.min, elements.max]).toEqual(["Elements", 1, 24]);
		expect(elements.read({ x: 1, y: 2.1, z: 0.6 })).toBe(8);
		expect(elements.write({ x: 1, y: 2.1, z: 0.6 }, 3).y).toBeCloseTo(0.85, 6);
	});

	it("offers a PA speaker its pole, none when it stands on its cabinet", () => {
		const [pole] = sizeMeasures(pa);
		expect([pole.label, pole.min, pole.max]).toEqual(["Pole", 0, 2]);
		expect(pole.read({ x: 0.35, y: 0.6, z: 0.4 })).toBe(0);
		expect(pole.write({ x: 0.35, y: 0.6, z: 0.4 }, 1.2).y).toBeCloseTo(1.8, 6);
		expect(pole.read({ x: 0.35, y: 1.8, z: 0.4 })).toBeCloseTo(1.2, 6);
		expect(pole.write({ x: 0.35, y: 1.8, z: 0.4 }, 0).y).toBe(0.6);
	});
});

describe("a disco ball measured by its diameter and its chain", () => {
	it("keeps the chain when the diameter changes, and the diameter when the chain does", () => {
		const [diameter, chain] = sizeMeasures(ball);
		expect([diameter.label, diameter.min, diameter.max]).toEqual(["Diameter", 0.2, 1.5]);
		expect([chain.label, chain.min, chain.max]).toEqual(["Chain", 0, 3]);
		const size = { x: 0.5, y: 0.75, z: 0.5 };
		expect(chain.read(size)).toBeCloseTo(0.25, 6);
		const bigger = diameter.write(size, 0.8);
		expect(bigger.x).toBe(0.8);
		expect(bigger.z).toBe(0.8);
		expect(bigger.y).toBeCloseTo(1.05, 6);
		expect(chain.write(size, 2).y).toBeCloseTo(2.5, 6);
	});

	it("draws the ball at the bottom of its drop with the chain up to the top", () => {
		expect(discoBallPlan(500, 500, "top_down")).toHaveLength(1);
		const [sphere, chain] = discoBallPlan(500, 2500, "front_to_back");
		const ys = sphere.points.map(([, y]) => y);
		expect(Math.min(...ys)).toBeCloseTo(-1250, 3);
		expect(Math.max(...ys)).toBeCloseTo(-750, 3);
		expect(Math.max(...chain.points.map(([, y]) => y))).toBe(1250);
		expect(Math.min(...chain.points.map(([, y]) => y))).toBe(-750);
		// Without a chain it is the ball alone.
		expect(discoBallPlan(500, 500, "left_to_right")).toHaveLength(1);
	});
});

describe("equipment on the plan", () => {
	it("draws a rack's units on its front, standing on the floor", () => {
		// The case and its four corner protectors, then a panel at each unit.
		const front = flightRackPlan(600, rackHeight(8), "front_to_back");
		expect(front).toHaveLength(5 + 8);
		expect(Math.min(...front.flatMap(({ points }) => points.map(([, y]) => y)))).toBe(0);
		// From the side it is the case alone; from above the case with its front edge.
		expect(flightRackPlan(600, rackHeight(8), "left_to_right")).toHaveLength(5);
		expect(flightRackPlan(600, 600, "top_down")).toHaveLength(6);
	});

	it("draws a rack as a road case with rounded, capped corners that fill its footprint in every view", () => {
		const extent = (points: readonly (readonly [number, number])[]) => [
			Math.min(...points.map(([x]) => x)),
			Math.max(...points.map(([x]) => x)),
			Math.min(...points.map(([, y]) => y)),
			Math.max(...points.map(([, y]) => y)),
		];
		for (const [width, height, view] of [
			[600, rackHeight(8), "front_to_back"],
			[800, rackHeight(16), "left_to_right"],
			[600, 800, "top_down"],
			[480, rackHeight(2), "back_to_front"],
		] as const) {
			const [shell, ...rest] = flightRackPlan(width, height, view);
			const bottom = view === "top_down" ? -height / 2 : 0;
			// The case spans exactly what it is placed at, so its selection box and snapping still fit it.
			for (const [actual, expected] of extent(shell.points).map((value, index) => [
				value,
				[-width / 2, width / 2, bottom, bottom + height][index],
			]))
				expect(actual).toBeCloseTo(expected, 6);
			// No point of the shell is a square corner: each corner is rounded off.
			for (const [x, y] of [
				[-width / 2, bottom],
				[width / 2, bottom],
				[width / 2, bottom + height],
				[-width / 2, bottom + height],
			])
				expect(shell.points.some(([px, py]) => Math.hypot(px - x, py - y) < 1)).toBe(false);
			expect(shell.points.length).toBeGreaterThan(4);
			// A protector sits over each corner, inside the footprint.
			const caps = rest.filter((polygon) => polygon.points.length > 4);
			expect(caps).toHaveLength(4);
			for (const cap of caps) {
				const [left, right, low, high] = extent(cap.points);
				expect(left).toBeGreaterThanOrEqual(-width / 2 - 1e-6);
				expect(right).toBeLessThanOrEqual(width / 2 + 1e-6);
				expect(low).toBeGreaterThanOrEqual(bottom - 1e-6);
				expect(high).toBeLessThanOrEqual(bottom + height + 1e-6);
			}
		}
	});

	it("puts a PA speaker on a pole and feet only when it has one", () => {
		expect(paSpeakerPlan(350, 600, "front_to_back")).toHaveLength(2);
		const poled = paSpeakerPlan(350, 1800, "front_to_back");
		expect(poled).toHaveLength(2 + 1 + 2);
		// The cabinet is at the top of its height.
		expect(Math.max(...poled[0].points.map(([, y]) => y))).toBe(1800);
		expect(Math.min(...poled[0].points.map(([, y]) => y))).toBe(1200);
	});

	it("hangs a line array's elements under its frame, about its middle", () => {
		const front = lineArrayPlan(1000, 2100, "front_to_back");
		expect(front).toHaveLength(1 + 2 * 8);
		const ys = front.flatMap(({ points }) => points.map(([, y]) => y));
		expect(Math.max(...ys)).toBeLessThanOrEqual(1050);
		expect(Math.min(...ys)).toBeGreaterThanOrEqual(-1050);
		expect(lineArrayPlan(1000, 850, "left_to_right")).toHaveLength(1 + 3);
	});
});
