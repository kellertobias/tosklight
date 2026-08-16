import { describe, expect, it } from "vitest";
import {
	entityPlanGeometry,
	parseProjection,
	projectionViewForCad,
} from "./projection";
import type { CadEntity } from "./types";

const movingLight: CadEntity = {
	id: "11111111-1111-4111-8111-111111111111",
	name: "Moving Head Profile",
	fixtureNumber: 101,
	fixtureDisplayId: "101",
	dmxAddress: "1.1",
	kind: "moving_head",
	fixtureType: "moving_head_profile",
	drawingId: "profile:1",
	layerId: "default",
	selectable: true,
	positionMillimetres: [0, 0, 4000],
	rotationDegrees: [0, 0, 0],
	sizeMillimetres: [400, 500, 700],
	outputDirection: [0, 1, 0],
};

describe("CAD plan projections", () => {
	it("maps every CAD direction to its fixture projection", () => {
		expect(projectionViewForCad("top_down")).toBe("top");
		expect(projectionViewForCad("left_to_right")).toBe("left");
		expect(projectionViewForCad("right_to_left")).toBe("right");
		expect(projectionViewForCad("front_to_back")).toBe("front");
		expect(projectionViewForCad("back_to_front")).toBe("back");
	});

	it("preserves opaque SVG paint order for covered model sections", () => {
		const geometry = parseProjection(`
			<svg><path d="M 0 0 L 10 0 L 10 10 L 0 10 Z" fill="#808080" />
			<path d="M 4 -2 L 6 -2 L 6 12 L 4 12 Z" fill="#202020" /></svg>
		`);

		expect(geometry.source).toBe("model");
		expect(geometry.triangles).toHaveLength(4);
		expect(geometry.triangles[0].color).toEqual([
			128 / 255,
			128 / 255,
			128 / 255,
		]);
		expect(geometry.triangles[3].color).toEqual([32 / 255, 32 / 255, 32 / 255]);
	});

	it("draws a representative moving-light side view when no model is available", () => {
		const geometry = entityPlanGeometry(
			movingLight,
			undefined,
			"left_to_right",
		);

		expect(geometry.source).toBe("typed");
		expect(geometry.triangles.length).toBeGreaterThan(20);
		const colors = geometry.triangles.map(({ color }) => color.join(","));
		const head = colors.indexOf("0.57,0.61,0.66");
		const nearYoke = colors.lastIndexOf("0.38,0.42,0.47");
		expect(head).toBeGreaterThan(-1);
		expect(nearYoke).toBeGreaterThan(head);
	});

	it("uses detailed lattice geometry for truss instead of a box", () => {
		const geometry = entityPlanGeometry(
			{
				...movingLight,
				name: "Four-Point Truss 4 m",
				kind: "venue",
				fixtureType: "truss",
				sizeMillimetres: [4000, 290, 290],
			},
			undefined,
			"front_to_back",
		);

		expect(geometry.source).toBe("typed");
		expect(geometry.outlines.length).toBeGreaterThan(4);
	});
});
