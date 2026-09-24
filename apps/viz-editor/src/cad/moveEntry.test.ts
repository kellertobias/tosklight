import { describe, expect, it } from "vitest";
import {
	applyMoveEntry,
	formatCoordinate,
	moveReadout,
	parseMoveEntry,
	worldAxisOf,
} from "./moveEntry";

describe("typed gizmo movement", () => {
	it("reads a bare number as a position and a signed one as a distance", () => {
		expect(parseMoveEntry("2.5")).toEqual({ mode: "absolute", metres: 2.5 });
		expect(parseMoveEntry("2,5")).toEqual({ mode: "absolute", metres: 2.5 });
		expect(parseMoveEntry("+.5")).toEqual({ mode: "relative", metres: 0.5 });
		expect(parseMoveEntry("-1.25")).toEqual({ mode: "relative", metres: -1.25 });
		for (const incomplete of ["", "+", "-", ".", "1..2", "1-2", "abc"])
			expect(parseMoveEntry(incomplete)).toBeNull();
	});

	it("sets or shifts only the active world axis and keeps the pointer's drag on the others", () => {
		const origin = [1000, -500, 4000] as const;
		const delta = [120, 80, 0] as const;
		expect(
			applyMoveEntry({ mode: "absolute", metres: 2 }, "x", origin, delta),
		).toEqual([1000, 80, 0]);
		expect(
			applyMoveEntry({ mode: "relative", metres: -0.5 }, "y", origin, delta),
		).toEqual([120, -500, 0]);
	});

	it("names the world axes each view shows, however a plan is turned", () => {
		expect(worldAxisOf("horizontal", "top_down", 0)).toBe("x");
		expect(worldAxisOf("horizontal", "top_down", 1)).toBe("y");
		expect(worldAxisOf("vertical", "left_to_right", 0)).toBe("z");
		const readout = moveReadout([0, 0, 4000], [0, 0, -4000.4], [0, 0], "front_to_back", 0, "vertical", "");
		expect(readout.coordinates.map((axis) => `${axis.label} ${axis.value}`)).toEqual([
			"X 0.000 m",
			"Z 0.000 m",
		]);
		expect(formatCoordinate(-1234.4)).toBe("-1.234 m");
	});
});
