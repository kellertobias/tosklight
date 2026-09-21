/**
 * Rotation has to be visible on the page, and has to say the same thing the Visualizer does.
 *
 * A drawing is a flat slice, so these check where its two axes land rather than where any one
 * point does: the axes are what carry pitch, roll and foreshortening.
 */
import { describe, expect, it } from "vitest";
import { planTransform } from "./planGeometry";
import type { PlanGeometry } from "./projection";
import { type CadEntity, projectPoint } from "./types";

const UNTURNED: PlanGeometry = {
	source: "typed",
	triangles: [],
	outlines: [],
	lines: [],
} as unknown as PlanGeometry;

function element(rotationDegrees: [number, number, number]): CadEntity {
	return {
		id: "one",
		logicalFixtureId: "one",
		positionMillimetres: [0, 0, 0],
		rotationDegrees,
		sizeMillimetres: [1000, 1000, 1000],
	} as unknown as CadEntity;
}

/** Where the drawing's across and up axes land on the page, rounded past floating-point dust. */
function axes(
	rotationDegrees: [number, number, number],
	view: Parameters<typeof planTransform>[2],
	rotationQuarterTurns = 0,
	geometry: PlanGeometry = UNTURNED,
) {
	const transform = planTransform(
		element(rotationDegrees),
		geometry,
		view,
		rotationQuarterTurns,
	);
	// Normalised past floating-point dust, and past negative zero, which is not a direction.
	const round = (point: readonly number[]) =>
		point.map((value) => (Math.round(value * 1000) || 0) / 1000);
	return { across: round(transform([1, 0])), up: round(transform([0, 1])) };
}

describe("an unturned element", () => {
	it("draws its slice exactly as it was built, in every view", () => {
		for (const view of [
			"top_down",
			"front_to_back",
			"back_to_front",
			"left_to_right",
			"right_to_left",
		] as const) {
			expect(axes([0, 0, 0], view)).toEqual({ across: [1, 0], up: [0, 1] });
		}
	});
});

describe("a lamp rolled 180 degrees about X", () => {
	it("hangs the other way up in an elevation, as it does in the Visualizer", () => {
		// The operator's case: pointing down in 2D while pointing up in 3D, because the body
		// symbol never received the roll and only the direction indicator did.
		expect(axes([180, 0, 0], "front_to_back").up).toEqual([0, -1]);
	});

	it("is seen from its other side looking down", () => {
		expect(axes([180, 0, 0], "top_down").up).toEqual([0, -1]);
	});
});

describe("a quarter turn about Z", () => {
	it("turns the drawing on the page looking down", () => {
		expect(axes([0, 0, 90], "top_down")).toEqual({ across: [0, 1], up: [-1, 0] });
	});

	it("is visible in an elevation too, by foreshortening the run to nothing", () => {
		// A 4 m truss turned side-on is drawn as its end, not as a 4 m run it no longer covers.
		expect(axes([0, 0, 90], "front_to_back").across).toEqual([0, 0]);
	});
});

describe("the page's own quarter turns", () => {
	it("turn a drawing the same way they turn where things stand", () => {
		// `rotatePlane` takes [1, 0] to [0, -1], and a drawing has to follow its own position.
		// The old page angle turned the other way, so on a quarter-turned plan every symbol was
		// skewed against the layout it belonged to.
		expect(axes([0, 0, 0], "top_down", 1)).toEqual({ across: [0, -1], up: [1, 0] });
	});

	it("compose with the element's own turn rather than replacing it", () => {
		// A quarter turn one way and a 90-degree yaw the other cancel exactly.
		expect(axes([0, 0, 90], "top_down", 1)).toEqual({ across: [1, 0], up: [0, 1] });
	});

	it("keep a drawing square to the positions around it", () => {
		for (const quarterTurns of [0, 1, 2, 3, -1]) {
			const { across } = axes([0, 0, 0], "top_down", quarterTurns);
			// Where the show's +X ends up on the page, from the projection positions use.
			const position = projectPoint([1000, 0, 0], "top_down", quarterTurns);
			expect(across).toEqual(position.map((value) => Math.round(value / 1000) || 0));
		}
	});
});

describe("an imported model", () => {
	it("keeps the axes it was projected with, having already been turned in three dimensions", () => {
		const live = { ...UNTURNED, source: "live_model" } as unknown as PlanGeometry;
		expect(axes([180, 0, 90], "front_to_back", 0, live)).toEqual({
			across: [1, 0],
			up: [0, 1],
		});
	});
});
