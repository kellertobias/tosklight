import { describe, expect, it } from "vitest";
import {
	NO_CUT_PLANES,
	depthExtent,
	entityDepthRange,
	viewDepth,
	visibleEntities,
	withinCutPlanes,
} from "./cutPlanes";

function entity(
	position: [number, number, number],
	size: [number, number, number] = [100, 100, 100],
	rotation: [number, number, number] = [0, 0, 0],
) {
	return {
		positionMillimetres: position,
		rotationDegrees: rotation,
		sizeMillimetres: size,
	};
}

describe("the axis a view looks along", () => {
	it("reads depth away from the viewer in every view", () => {
		// Looking down: the grid is nearer than the floor.
		expect(viewDepth([0, 0, 6_000], "top_down")).toBeLessThan(
			viewDepth([0, 0, 0], "top_down"),
		);
		expect(viewDepth([2_000, 0, 0], "left_to_right")).toBe(2_000);
		expect(viewDepth([2_000, 0, 0], "right_to_left")).toBe(-2_000);
		expect(viewDepth([0, 2_000, 0], "front_to_back")).toBe(2_000);
		expect(viewDepth([0, 2_000, 0], "back_to_front")).toBe(-2_000);
	});

	it("gives an element the thickness it has along that axis", () => {
		const range = entityDepthRange(
			entity([1_000, 0, 0], [400, 100, 100]),
			"left_to_right",
		);
		expect(range).toEqual({ near: 800, far: 1_200 });
	});

	it("gives a turned element the thickness it presents once turned", () => {
		const turned = entityDepthRange(
			entity([0, 0, 0], [4_000, 200, 200], [0, 90, 0]),
			"left_to_right",
		);
		// Swung a quarter turn, the long truss now runs across the view rather than into it.
		expect(turned.far - turned.near).toBeCloseTo(200);
	});
});

describe("a slice of the drawing", () => {
	const upstage = entity([4_000, 0, 0]);
	const midstage = entity([0, 0, 0]);
	const downstage = entity([-4_000, 0, 0]);
	const all = [downstage, midstage, upstage];

	it("shows everything when neither end is set", () => {
		expect(visibleEntities(all, "left_to_right", NO_CUT_PLANES)).toBe(all);
		expect(visibleEntities(all, "left_to_right", undefined)).toBe(all);
	});

	it("drops what lies beyond a far cut", () => {
		expect(
			visibleEntities(all, "left_to_right", {
				nearMillimetres: null,
				farMillimetres: 1_000,
			}),
		).toEqual([downstage, midstage]);
	});

	it("drops what lies in front of a near cut", () => {
		expect(
			visibleEntities(all, "left_to_right", {
				nearMillimetres: -1_000,
				farMillimetres: null,
			}),
		).toEqual([midstage, upstage]);
	});

	it("keeps only the slice between the two", () => {
		expect(
			visibleEntities(all, "left_to_right", {
				nearMillimetres: -1_000,
				farMillimetres: 1_000,
			}),
		).toEqual([midstage]);
	});

	it("reads a pair given the wrong way round as the same slice", () => {
		expect(
			visibleEntities(all, "left_to_right", {
				nearMillimetres: 1_000,
				farMillimetres: -1_000,
			}),
		).toEqual([midstage]);
	});

	it("keeps an element the cut passes through, because it has thickness", () => {
		const wide = entity([0, 0, 0], [4_000, 100, 100]);
		expect(
			withinCutPlanes(entityDepthRange(wide, "left_to_right"), {
				nearMillimetres: 1_500,
				farMillimetres: 3_000,
			}),
		).toBe(true);
	});

	it("cuts the same drawing the other way round when the view is reversed", () => {
		// The same far cut from the opposite side keeps the opposite elements.
		expect(
			visibleEntities(all, "right_to_left", {
				nearMillimetres: null,
				farMillimetres: 1_000,
			}),
		).toEqual([midstage, upstage]);
	});
});

describe("the depth a drawing spans", () => {
	it("reports the near and far ends a control can offer", () => {
		expect(
			depthExtent(
				[entity([-4_000, 0, 0]), entity([4_000, 0, 0])],
				"left_to_right",
			),
		).toEqual({ near: -4_050, far: 4_050 });
	});

	it("reports nothing for an empty drawing", () => {
		expect(depthExtent([], "top_down")).toBeNull();
	});
});
