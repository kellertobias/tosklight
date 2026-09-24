import type { FixtureProfileScenery } from "@tosklight/patch";
import { describe, expect, it } from "vitest";
import { flightRackPlan, lineArrayPlan, paSpeakerPlan, rackHeight } from "./equipmentPlan";
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

describe("equipment on the plan", () => {
	it("draws a rack's units on its front, standing on the floor", () => {
		const front = flightRackPlan(600, rackHeight(8), "front_to_back");
		expect(front).toHaveLength(1 + 8);
		expect(Math.min(...front.flatMap(({ points }) => points.map(([, y]) => y)))).toBe(0);
		// From the side it is the case alone; from above the box with its front edge.
		expect(flightRackPlan(600, rackHeight(8), "left_to_right")).toHaveLength(1);
		expect(flightRackPlan(600, 600, "top_down")).toHaveLength(2);
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
