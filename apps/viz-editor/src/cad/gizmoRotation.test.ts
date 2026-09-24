import { describe, expect, it } from "vitest";
import {
	hitsRotateArc,
	pageTurnSign,
	rotateArc,
	rotationAxisIndex,
	snappedTurn,
	turnedPlacements,
	withTurnedPlacements,
} from "./gizmoRotation";
import { rotateDeskPoint } from "./projection";
import type { CadEntity, CadViewDirection } from "./types";
import { projectPoint } from "./types";

const lamp = (id: string, position: [number, number, number]): CadEntity =>
	({
		id,
		logicalFixtureId: id,
		positionMillimetres: position,
		rotationDegrees: [0, 0, 170],
		sizeMillimetres: [400, 400, 400],
		selectable: true,
	}) as CadEntity;

describe("the gizmo's rotate handle", () => {
	it("turns about the axis each view looks along", () => {
		expect(rotationAxisIndex("top_down")).toBe(2);
		expect(rotationAxisIndex("front_to_back")).toBe(1);
		expect(rotationAxisIndex("back_to_front")).toBe(1);
		expect(rotationAxisIndex("left_to_right")).toBe(0);
		expect(rotationAxisIndex("right_to_left")).toBe(0);
	});

	it("turns things the way the pointer sweeps on every page", () => {
		const views: CadViewDirection[] = [
			"top_down",
			"front_to_back",
			"back_to_front",
			"left_to_right",
			"right_to_left",
		];
		for (const view of views)
			for (const quarter of [0, 1, 2, 3]) {
				// A positive page sweep, carried by the sign, turns a point anticlockwise on the page.
				const axis = rotationAxisIndex(view);
				const turn: [number, number, number] = [0, 0, 0];
				turn[axis] = 20 * pageTurnSign(view, quarter);
				const probe: [number, number, number] = axis === 0 ? [0, 1000, 0] : [1000, 0, 0];
				const before = projectPoint(probe, view, quarter);
				const after = projectPoint(rotateDeskPoint(probe, turn), view, quarter);
				expect(before[0] * after[1] - before[1] * after[0], `${view} ${quarter}`).toBeGreaterThan(0);
			}
	});

	it("is a quarter arc between the arrows that a press near it takes", () => {
		const arc = rotateArc([0, 0], 480);
		expect(arc.length).toBeGreaterThan(8);
		for (const [x, y] of arc) {
			expect(x).toBeGreaterThan(0);
			expect(y).toBeGreaterThan(0);
		}
		const middle = arc[Math.floor(arc.length / 2)];
		expect(hitsRotateArc(middle, [0, 0], 480, 0.1)).toBe(true);
		// Eight pixels either side at any zoom, never the arrows or the pivot.
		expect(hitsRotateArc([middle[0] * 1.5, middle[1] * 1.5], [0, 0], 480, 0.1)).toBe(false);
		expect(hitsRotateArc([400, 0], [0, 0], 480, 0.1)).toBe(false);
		expect(hitsRotateArc([0, 0], [0, 0], 480, 0.1)).toBe(false);
	});

	it("snaps a turn to 15° steps unless it turns freely", () => {
		expect(snappedTurn(22, false)).toBe(15);
		expect(snappedTurn(23, false)).toBe(30);
		expect(snappedTurn(22.34, true)).toBe(22.3);
		expect(snappedTurn(350, false)).toBe(-15);
	});

	it("carries several fixtures round the pivot and turns each by the same amount", () => {
		const a = lamp("a", [1000, 0, 5000]);
		const b = lamp("b", [-1000, 0, 5000]);
		const copy = { ...lamp("a-copy", [3000, 0, 5000]), logicalFixtureId: "a" };
		const placements = turnedPlacements([a, b, copy], ["a", "b"], [0, 0, 5000], 2, 90);
		expect(placements.map(({ id }) => id)).toEqual(["a", "b"]);
		const [turnedA, turnedB] = placements;
		expect(turnedA.positionMillimetres).toEqual(
			rotateDeskPoint([1000, 0, 0], [0, 0, 90]).map((value, index) => Math.round(value + [0, 0, 5000][index])),
		);
		expect(turnedB.positionMillimetres[0]).toBe(-turnedA.positionMillimetres[0]);
		// 170° + 90° wraps round to -100°.
		expect(turnedA.rotationDegrees).toEqual([0, 0, -100]);
		// The preview draws each turned fixture in place of where it stood, and nothing else.
		const drawn = withTurnedPlacements([a, b, copy], placements);
		expect(drawn[0].positionMillimetres).toEqual(turnedA.positionMillimetres);
		expect(drawn[2]).toBe(copy);
		expect(withTurnedPlacements([a], undefined)[0]).toBe(a);
	});
});
