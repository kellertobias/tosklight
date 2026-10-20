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

it("retains a joined wall's actual miter contour alongside its opening frames", () => {
	if (!wall.venueGeometry) throw new Error("Missing native wall");
	const joined: CadEntity = {
		...wall,
		venueGeometry: {
			...wall.venueGeometry,
			joinedWall: true,
			exterior: [
				[
					[-2000, 0, -100],
					[2100, 0, -100],
				],
				[
					[2100, 0, -100],
					[2100, 3000, -100],
				],
				[
					[2100, 3000, -100],
					[-2000, 3000, -100],
				],
				[
					[-2000, 3000, -100],
					[-2000, 0, -100],
				],
			],
			elevation: wall.venueGeometry.elevation?.slice(4),
			triangles: wall.venueGeometry.triangles.map(
				(triangle) =>
					triangle.map(([x, y, z]) => [x === 2000 ? 2100 : x, y, z]) as [
						number[],
						number[],
						number[],
					],
			),
		},
	};
	const geometry = entityPlanGeometry(joined, undefined, "front_to_back");
	expect(
		geometry.lines.some((line) =>
			line.points.some((point) => point[0] === 2100),
		),
	).toBe(true);
	const frames = geometry.lines.slice(-4);
	expect(frames.map((line) => line.points)).toEqual([
		[
			[-500, 1000],
			[500, 1000],
		],
		[
			[500, 1000],
			[500, 2000],
		],
		[
			[500, 2000],
			[-500, 2000],
		],
		[
			[-500, 2000],
			[-500, 1000],
		],
	]);
	expect(
		frames.every((line) => line.depths?.every((depth) => depth === -100)),
	).toBe(true);
	expect(
		pickEntity([1100, 2100], [joined], new Map(), "front_to_back", 0, {
			pan: [0, 0],
			zoom: 1,
		}),
	).toBeNull();
	expect(
		pickEntity([3090, 2100], [joined], new Map(), "front_to_back", 0, {
			pan: [0, 0],
			zoom: 1,
		})?.id,
	).toBe(joined.id);
});
it("suppresses coplanar aperture-strip seams in joined wall elevations", () => {
	const joined: CadEntity = {
		...wall,
		venueGeometry: {
			...wall.venueGeometry!,
			joinedWall: true,
			elevation: [],
			exterior: [
				[
					[-2000, 0, -100],
					[2000, 0, -100],
				],
				[
					[-2000, 3000, -100],
					[2000, 3000, -100],
				],
			],
		},
	};
	const lines = entityPlanGeometry(joined, undefined, "front_to_back").lines;
	expect(lines.length).toBeGreaterThan(0);
	expect(
		lines.some(
			({ points: [a, b] }) =>
				Math.abs(a[0]) === 500 &&
				a[0] === b[0] &&
				Math.min(a[1], b[1]) === 0 &&
				Math.max(a[1], b[1]) === 1000,
		),
	).toBe(false);
	expect(
		lines.some(
			({ points: [a, b] }) =>
				Math.abs(a[0]) === 500 &&
				a[0] === b[0] &&
				Math.min(a[1], b[1]) === 2000 &&
				Math.max(a[1], b[1]) === 3000,
		),
	).toBe(false);
});
it("preserves a door's floor gap from the native joined exterior", () => {
	const joined: CadEntity = {
		...wall,
		venueGeometry: {
			joinedWall: true,
			plan: [],
			elevation: [],
			exterior: [
				[
					[-2000, 0, -100],
					[-500, 0, -100],
				],
				[
					[500, 0, -100],
					[2000, 0, -100],
				],
				[
					[-500, 2100, -100],
					[500, 2100, -100],
				],
			],
			triangles: [
				...strip(-2000, -500, 0, 3000),
				...strip(500, 2000, 0, 3000),
				...strip(-500, 500, 2100, 3000),
			],
		},
	};
	const lines = entityPlanGeometry(joined, undefined, "front_to_back").lines;
	expect(
		lines.some(
			({ points: [a, b] }) =>
				a[1] === 0 &&
				b[1] === 0 &&
				Math.min(a[0], b[0]) < 0 &&
				Math.max(a[0], b[0]) > 0,
		),
	).toBe(false);
	expect(
		lines.some(({ points: [a, b] }) => a[1] === 2100 && b[1] === 2100),
	).toBe(true);
});
it("keeps actual partial-height steps instead of replacing them with a bounding rectangle", () => {
	const joined: CadEntity = {
		...wall,
		venueGeometry: {
			joinedWall: true,
			plan: [],
			elevation: [],
			exterior: [
				[
					[0, 1500, -100],
					[0, 3000, -100],
				],
				[
					[0, 1500, -100],
					[2000, 1500, -100],
				],
			],
			triangles: [...strip(-2000, 0, 0, 3000), ...strip(0, 2000, 0, 1500)],
		},
	};
	const lines = entityPlanGeometry(joined, undefined, "front_to_back").lines;
	expect(
		lines.some(
			({ points: [a, b] }) =>
				a[0] === 0 &&
				b[0] === 0 &&
				Math.min(a[1], b[1]) === 1500 &&
				Math.max(a[1], b[1]) === 3000,
		),
	).toBe(true);
	expect(
		lines.some(
			({ points: [a, b] }) =>
				a[0] === 0 && b[0] === 0 && Math.max(a[1], b[1]) <= 1500,
		),
	).toBe(false);
});
