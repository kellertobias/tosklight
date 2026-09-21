import { beforeEach, describe, expect, it } from "vitest";
import {
	DUPLICATE_FALLBACK_MILLIMETRES,
	duplicateStep,
	forgetMoveAxes,
	rememberMoveAxis,
} from "./duplicateStep";
import type { CadEntity } from "./types";

function element(id: string, size: [number, number, number], x = 0): CadEntity {
	return {
		id,
		logicalFixtureId: id,
		positionMillimetres: [x, 0, 0],
		rotationDegrees: [0, 0, 0],
		sizeMillimetres: size,
	} as unknown as CadEntity;
}

beforeEach(forgetMoveAxes);

describe("the step a duplicate takes", () => {
	it("is the element's own width, so the copy lands beside it whatever its size", () => {
		const deck = element("deck", [2000, 1000, 200]);
		expect(duplicateStep([deck], ["deck"], "top_down", 0)).toEqual([2000, 0, 0]);
		const truss = element("truss", [4000, 290, 290]);
		expect(duplicateStep([truss], ["truss"], "top_down", 0)).toEqual([4000, 0, 0]);
	});

	it("spans the whole selection, so elements copied together keep their arrangement", () => {
		const left = element("left", [1000, 1000, 200], 0);
		const right = element("right", [1000, 1000, 200], 3000);
		// 0 - 500 up to 3000 + 500: the pair steps clear of itself rather than landing on top.
		expect(duplicateStep([left, right], ["left", "right"], "top_down", 0)).toEqual([4000, 0, 0]);
	});

	it("follows the axis that element was last moved along", () => {
		const deck = element("deck", [2000, 1000, 200]);
		rememberMoveAxis(["deck"], [0, 1500, 0], "top_down", 0);
		const step = duplicateStep([deck], ["deck"], "top_down", 0);
		expect(step[0]).toBe(0);
		// Up the page in top-down is the depth axis, and the deck is 1000 deep.
		expect(Math.abs(step[1])).toBe(1000);
	});

	it("goes along the view's right when the selection's members disagree", () => {
		const left = element("left", [1000, 1000, 200], 0);
		const right = element("right", [1000, 1000, 200], 3000);
		rememberMoveAxis(["left"], [0, 1500, 0], "top_down", 0);
		rememberMoveAxis(["right"], [1500, 0, 0], "top_down", 0);
		expect(duplicateStep([left, right], ["left", "right"], "top_down", 0)).toEqual([4000, 0, 0]);
	});

	it("ignores a move that went nowhere in the drawing plane", () => {
		const deck = element("deck", [2000, 1000, 200]);
		// Straight up, seen from above: it says nothing about which way the operator is working.
		rememberMoveAxis(["deck"], [0, 0, 2000], "top_down", 0);
		expect(duplicateStep([deck], ["deck"], "top_down", 0)).toEqual([2000, 0, 0]);
	});

	it("falls back to a fixed step when there is nothing to measure", () => {
		expect(duplicateStep([], ["gone"], "top_down", 0)).toEqual([
			DUPLICATE_FALLBACK_MILLIMETRES,
			0,
			0,
		]);
	});
});
