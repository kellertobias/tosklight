import { expect, it } from "vitest";
import { pickEntity, worldGeometry } from "./planGeometry";
import { entityPlanGeometry } from "./projection";
import type { CadEntity } from "./types";

type Point = [number, number, number];
function quad(a: Point, b: Point, c: Point, d: Point): [Point, Point, Point][] {
	return [
		[a, b, c],
		[a, c, d],
	];
}
function strip(
	x1: number,
	x2: number,
	y1: number,
	y2: number,
): [Point, Point, Point][] {
	return quad([x1, y1, -100], [x2, y1, -100], [x2, y2, -100], [x1, y2, -100]);
}
const wall: CadEntity = {
	id: "window-wall",
	logicalFixtureId: "window-wall",
	name: "Window wall",
	fixtureNumber: null,
	fixtureDisplayId: "0.1",
	dmxAddress: "—",
	kind: "venue",
	fixtureType: "venue",
	drawingId: "wall",
	layerId: "default",
	selectable: true,
	positionMillimetres: [1000, 2000, 500],
	rotationDegrees: [0, 0, 0],
	sizeMillimetres: [4000, 200, 3000],
	outputDirection: [0, 0, 0],
	scenery: {
		kind: "box",
		chords: 0,
		pattern: "standard",
		architecture: { kind: "wall", openings: [] },
	},
	venueGeometry: {
		triangles: [
			...strip(-2000, -500, 0, 3000),
			...strip(500, 2000, 0, 3000),
			...strip(-500, 500, 0, 1000),
			...strip(-500, 500, 2000, 3000),
			...quad(
				[-2000, 0, -100],
				[-2000, 3000, -100],
				[-2000, 3000, 100],
				[-2000, 0, 100],
			),
			...quad(
				[2000, 0, -100],
				[2000, 0, 100],
				[2000, 3000, 100],
				[2000, 3000, -100],
			),
			...quad(
				[-2000, 0, -100],
				[-2000, 0, 100],
				[2000, 0, 100],
				[2000, 0, -100],
			),
			...quad(
				[-2000, 3000, -100],
				[2000, 3000, -100],
				[2000, 3000, 100],
				[-2000, 3000, 100],
			),
		],
		plan: [
			[
				[-2000, -100],
				[2000, -100],
			],
			[
				[2000, -100],
				[2000, 100],
			],
			[
				[2000, 100],
				[-2000, 100],
			],
			[
				[-2000, 100],
				[-2000, -100],
			],
		],
		elevation: [
			[
				[-2000, 0, 0],
				[2000, 0, 0],
			],
			[
				[2000, 0, 0],
				[2000, 3000, 0],
			],
			[
				[2000, 3000, 0],
				[-2000, 3000, 0],
			],
			[
				[-2000, 3000, 0],
				[-2000, 0, 0],
			],
			[
				[-500, 1000, 0],
				[500, 1000, 0],
			],
			[
				[500, 1000, 0],
				[500, 2000, 0],
			],
			[
				[500, 2000, 0],
				[-500, 2000, 0],
			],
			[
				[-500, 2000, 0],
				[-500, 1000, 0],
			],
		],
	},
};
function withoutFrame(entity: CadEntity): CadEntity {
	if (!entity.venueGeometry) throw new Error("Expected venue geometry");
	return {
		...entity,
		venueGeometry: { ...entity.venueGeometry, elevation: undefined },
	};
}
it("uses only the window frame and wall outline in a face-on elevation, without aperture-subtraction seams", () => {
	const geometry = entityPlanGeometry(wall, undefined, "front_to_back");
	expect(geometry.lines.map((line) => line.points)).toEqual(
		wall.venueGeometry?.elevation?.map((line) => line.map(([x, y]) => [x, y])),
	);
	expect(geometry.outlines).toEqual([]);
	expect(geometry.lines).toHaveLength(8);
	expect(geometry.lines).not.toEqual(
		entityPlanGeometry(withoutFrame(wall), undefined, "front_to_back").lines,
	);
});
it("keeps the frame as linework and leaves the actual window opening empty for picking", () => {
	const geometry = entityPlanGeometry(wall, undefined, "front_to_back");
	expect(geometry.triangles.length).toBeGreaterThan(0);
	expect(
		pickEntity([1100, 2100], [wall], new Map(), "front_to_back", 0, {
			pan: [0, 0],
			zoom: 1,
		}),
	).toBeNull();
});
it("turns semantic frame coordinates with wall yaw and adds the placement only once", () => {
	const turned = {
		...wall,
		rotationDegrees: [0, 0, 90] as [number, number, number],
	};
	const geometry = entityPlanGeometry(turned, undefined, "left_to_right");
	expect(geometry.lines[0].points[0][0]).toBeCloseTo(2000);
	expect(geometry.lines[0].points[1][0]).toBeCloseTo(-2000);
	const world = worldGeometry(turned, geometry, "left_to_right", 0);
	expect(world.lines[0].points[0][0]).toBeCloseTo(0);
	expect(world.lines[0].points[0][1]).toBeCloseTo(500);
});
it("retains physical thickness linework in an edge-on side view", () => {
	expect(entityPlanGeometry(wall, undefined, "left_to_right").lines).toEqual(
		entityPlanGeometry(withoutFrame(wall), undefined, "left_to_right").lines,
	);
});
it("detects a rolled edge-on wall from the projected face area rather than horizontal width alone", () => {
	const flat = {
		...wall,
		rotationDegrees: [90, 0, 0] as [number, number, number],
	};
	expect(entityPlanGeometry(flat, undefined, "front_to_back").lines).toEqual(
		entityPlanGeometry(withoutFrame(flat), undefined, "front_to_back").lines,
	);
});
it("still uses frame symbols when a face-on wall's local axes are exchanged by rotation", () => {
	const turned = {
		...wall,
		rotationDegrees: [0, 90, 0] as [number, number, number],
	};
	const geometry = entityPlanGeometry(turned, undefined, "front_to_back");
	expect(geometry.lines).toHaveLength(8);
	expect(geometry.outlines).toEqual([]);
	expect(
		geometry.lines[0].points[1][1] - geometry.lines[0].points[0][1],
	).toBeCloseTo(-4000);
});
it("keeps the shared top plan and legacy geometry without semantic elevation unchanged", () => {
	const top = entityPlanGeometry(wall, undefined, "top_down");
	expect(top.lines.map((line) => line.points)).toEqual(
		wall.venueGeometry?.plan.map((line) => line.map(([x, z]) => [x, -z])),
	);
	expect(top.triangles).toEqual([]);
	expect(top.pickTriangles).toBeUndefined();
	expect(
		entityPlanGeometry(withoutFrame(wall), undefined, "front_to_back").lines
			.length,
	).toBeGreaterThan(0);
});

it("places frame endpoints on the nearest physical wall face with actual view depths", () => {
	for (const view of ["front_to_back", "back_to_front"] as const) {
		const geometry = entityPlanGeometry(wall, undefined, view);
		expect(
			geometry.lines.every((line) =>
				line.depths?.every((depth) => Math.abs(depth + 100) < 1e-6),
			),
		).toBe(true);
	}
	const turned = {
		...wall,
		rotationDegrees: [0, 0, 30] as [number, number, number],
	};
	const geometry = entityPlanGeometry(turned, undefined, "front_to_back");
	// At an oblique view the nearest face contributes -100 cos(30°), while the two
	// endpoints carry opposite width-axis depth offsets. They cannot share depth zero.
	expect(geometry.lines[0].depths?.[0]).toBeCloseTo(
		-1000 - Math.cos(Math.PI / 6) * 100,
	);
	expect(geometry.lines[0].depths?.[1]).toBeCloseTo(
		1000 - Math.cos(Math.PI / 6) * 100,
	);
});
