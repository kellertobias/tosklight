/**
 * The bracket angle on the shipped Fresnel's real drawings: the body turns about the hinge its
 * manifest records while the hanging frame and coupler stay put, in whichever elevation shows the
 * lamp's side, by the same turn the Visualizer gives it (`bracketTurnedPoint`, `Rx(bracket)`).
 */
import { describe, expect, it } from "vitest";
import {
	lampRelativeView,
	modelDrawingGeometry,
	parseModelDrawing,
} from "./modelDrawing";
import { bracketTurnedPoint, entityPlanGeometry, type PlanPoint } from "./projection";
import type { CadDrawing, CadEntity, CadViewDirection } from "./types";
import FRONT from "../../../../assets/models/2d/lamps/fresnel-barn-doors/front.svg?raw";
import SIDE from "../../../../assets/models/2d/lamps/fresnel-barn-doors/side.svg?raw";
import TOP from "../../../../assets/models/2d/lamps/fresnel-barn-doors/top.svg?raw";

const fresnel: CadDrawing = {
	id: "fresnel:1",
	projections: [],
	modelDrawing: {
		model: "fresnel-barn-doors",
		scale: 1,
		views: [
			{ view: "top", svg: TOP },
			{ view: "front", svg: FRONT },
			{ view: "side", svg: SIDE },
		],
	},
};

const lamp: CadEntity = {
	id: "fresnel",
	logicalFixtureId: "fresnel",
	name: "Fresnel",
	fixtureNumber: 1,
	fixtureDisplayId: "1",
	dmxAddress: "1.1",
	kind: "fixture",
	fixtureType: "fresnel",
	drawingId: "fresnel:1",
	layerId: "default",
	selectable: true,
	positionMillimetres: [0, 0, 6000],
	rotationDegrees: [0, 0, 0],
	sizeMillimetres: [450, 500, 900],
	outputDirection: [0, 0, -1],
	bracketAngle: 0,
};

const parsed = parseModelDrawing(SIDE);
const hinge = parsed.hinge as PlanPoint;

const geometry = (
	bracketAngle: number,
	view: CadViewDirection,
	yaw = 0,
	entity: CadEntity = lamp,
) =>
	entityPlanGeometry(
		{ ...entity, bracketAngle, rotationDegrees: [0, 0, yaw] },
		fresnel,
		view,
	);

/**
 * Where a side-drawing point lands once the Visualizer turns the body `degrees` about the hinge.
 * The side page is (model z, model y); the hinge sits on the lamp's centre line.
 */
const turnedLikeTheVisualizer = ([z, y]: PlanPoint, degrees: number): PlanPoint => {
	const [, turnedY, turnedZ] = bracketTurnedPoint([0, y - hinge[1], z - hinge[0]], degrees);
	return [turnedZ + hinge[0], turnedY + hinge[1]];
};

const close = (left: PlanPoint, right: PlanPoint) =>
	Math.abs(left[0] - right[0]) < 1e-6 && Math.abs(left[1] - right[1]) < 1e-6;

describe("a Fresnel at a 45° bracket angle in CAD", () => {
	it("records its hinge and keeps its hanging hardware apart from the body", () => {
		expect(hinge).toEqual([0, -353]);
		expect(parsed.bracket).toBeUndefined();
		expect(parsed.body.triangles.length).toBeGreaterThan(0);
		expect(parsed.hardware.triangles.length).toBeGreaterThan(0);
		expect(parsed.hardware.lines.length).toBeGreaterThan(0);
	});

	it("turns every body point 45° about the hinge, as the Visualizer does, and leaves the bracket", () => {
		const turned = geometry(45, "left_to_right");
		const bodyCount = parsed.body.triangles.length;
		expect(turned.triangles).toHaveLength(bodyCount + parsed.hardware.triangles.length);
		turned.triangles.slice(0, bodyCount).forEach((triangle, index) => {
			triangle.points.forEach((point, corner) => {
				const drawn = parsed.body.triangles[index][corner];
				expect(close(point, turnedLikeTheVisualizer(drawn, 45))).toBe(true);
			});
		});
		turned.triangles.slice(bodyCount).forEach((triangle, index) => {
			expect(triangle.points).toEqual(parsed.hardware.triangles[index]);
		});
		// The body's lowest point, the lens end, swings 45° on its arm about the hinge.
		const lowest = parsed.body.triangles
			.flat()
			.reduce((low, point) => (point[1] < low[1] ? point : low));
		const moved = turnedLikeTheVisualizer(lowest, 45);
		const before = Math.atan2(lowest[1] - hinge[1], lowest[0] - hinge[0]);
		const after = Math.atan2(moved[1] - hinge[1], moved[0] - hinge[0]);
		// Nose-down is clockwise on the page, with plan y up.
		expect(((before - after) * 180) / Math.PI).toBeCloseTo(45, 6);
		// Nose-down turns the lens upstage (model −z), as in the 3D view.
		expect(moved[0]).toBeLessThan(lowest[0]);
	});

	it("changes the side projection when the bracket angle changes", () => {
		const level = geometry(0, "left_to_right");
		const turned = geometry(45, "left_to_right");
		expect(level.triangles[0].points).toEqual(parsed.body.triangles[0]);
		expect(turned.triangles[0].points).not.toEqual(level.triangles[0].points);
	});

	it("shows the turned side in whichever elevation faces the lamp's side", () => {
		const side = geometry(45, "left_to_right").triangles;
		const mirrored = geometry(45, "right_to_left").triangles;
		// Yawed a quarter turn, the front elevation looks at the lamp's left side.
		expect(lampRelativeView("front_to_back", 90)).toBe("left_to_right");
		expect(geometry(45, "front_to_back", 90).triangles).toEqual(side);
		// The other way round, and from behind, it sees the right side, mirrored.
		expect(lampRelativeView("front_to_back", -90)).toBe("right_to_left");
		expect(geometry(45, "front_to_back", -90).triangles).toEqual(mirrored);
		expect(geometry(45, "back_to_front", 90).triangles).toEqual(mirrored);
		// Yawed a half turn, the side elevations swap.
		expect(geometry(45, "left_to_right", 180).triangles).toEqual(mirrored);
		// Unyawed, the front elevation shows the front drawing as drawn.
		const front = parseModelDrawing(FRONT);
		expect(geometry(45, "front_to_back").triangles).toHaveLength(front.triangles.length);
		expect(geometry(45, "front_to_back").triangles[0].points).toEqual(
			front.triangles[0],
		);
		// Yaw rounds to the nearest quarter turn; the top view keeps its own yaw handling.
		expect(lampRelativeView("left_to_right", 30)).toBe("left_to_right");
		expect(lampRelativeView("left_to_right", 60)).toBe("back_to_front");
		expect(lampRelativeView("top_down", 90)).toBe("top_down");
	});

	it("draws a saved 45° angle the same after the plan reloads, and an older plan level", () => {
		const reloaded = JSON.parse(
			JSON.stringify({ ...lamp, bracketAngle: 45 }),
		) as CadEntity;
		expect(geometry(45, "left_to_right", 0, reloaded)).toEqual(
			geometry(45, "left_to_right"),
		);
		const { bracketAngle: _, ...older } = lamp;
		expect(
			entityPlanGeometry(older as CadEntity, fresnel, "left_to_right"),
		).toEqual(geometry(0, "left_to_right"));
		expect(
			modelDrawingGeometry(fresnel, "left_to_right")?.triangles[0].points,
		).toEqual(parsed.body.triangles[0]);
	});
});

describe("a lamp drawn from its own model", () => {
	const live: CadDrawing = {
		id: "live:1",
		projections: [],
		liveMeshes: [
			{
				pose: "elevation",
				triangles: [
					{
						pointsMillimetres: [
							[0, -500, 0],
							[0, -300, 0],
							[0, -500, 100],
						],
						colour: [0.1, 0.1, 0.1],
					},
				],
			},
		],
	};

	it("turns its projected mesh by the bracket about its origin, as the Visualizer does", () => {
		const entity = { ...lamp, drawingId: "live:1" };
		const level = entityPlanGeometry({ ...entity, bracketAngle: 0 }, live, "left_to_right");
		const turned = entityPlanGeometry({ ...entity, bracketAngle: 45 }, live, "left_to_right");
		expect(level.source).toBe("live_model");
		const lens = (points: readonly PlanPoint[]) => points[0];
		expect(lens(level.triangles[0].points)).toEqual([0, -500]);
		const [, y, z] = bracketTurnedPoint([0, -500, 0], 45);
		expect(close(lens(turned.triangles[0].points), [z, y])).toBe(true);
		expect(z).toBeCloseTo(-500 * Math.sin(Math.PI / 4), 6);
	});
});
