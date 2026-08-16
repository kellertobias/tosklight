import { describe, expect, it } from "vitest";
import { audienceOutline } from "./audienceOutline";
import {
	audiencePersonHeight,
	entityPlanGeometry,
	parseProjection,
	projectionViewForCad,
} from "./projection";
import type { CadDrawing, CadEntity } from "./types";

const movingLight: CadEntity = {
	id: "11111111-1111-4111-8111-111111111111",
	logicalFixtureId: "11111111-1111-4111-8111-111111111111",
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

	it("live-projects a fixture 151 equivalent after its complete compound rotation", () => {
		const drawing: CadDrawing = {
			id: "profile:151",
			projections: [],
			liveMeshes: ["top", "elevation"].map((pose) => ({
				pose: pose as "top" | "elevation",
				triangles: [
					{
						pointsMillimetres: [
							[-180, -120, -80],
							[220, -80, -40],
							[-100, 160, 40],
						],
						colour: [0.8, 0.2, 0.2] as [number, number, number],
					},
					{
						pointsMillimetres: [
							[-60, -40, 160],
							[160, 20, 200],
							[20, 220, 260],
						],
						colour: [0.2, 0.8, 0.2] as [number, number, number],
					},
				],
			})),
		};
		const fixture151 = {
			...movingLight,
			fixtureNumber: 151,
			fixtureDisplayId: "151",
			rotationDegrees: [17, 29, 41] as [number, number, number],
		};
		const views = ["top_down", "left_to_right", "front_to_back"] as const;
		const geometries = views.map((view) =>
			entityPlanGeometry(fixture151, drawing, view),
		);

		expect(geometries.every(({ source }) => source === "live_model")).toBe(
			true,
		);
		expect(geometries.every(({ triangles }) => triangles.length === 2)).toBe(
			true,
		);
		expect(geometries.every(({ lines }) => lines.length > 0)).toBe(true);
		expect(
			geometries.every(({ triangles }) =>
				triangles.every(
					({ depths }) => depths?.length === 3 && depths.every(Number.isFinite),
				),
			),
		).toBe(true);
		const bounds = geometries.map(({ triangles }) => {
			const points = triangles.flatMap(({ points }) => points);
			return [
				Math.min(...points.map(([x]) => x)),
				Math.max(...points.map(([x]) => x)),
				Math.min(...points.map(([, y]) => y)),
				Math.max(...points.map(([, y]) => y)),
			].map((value) => Math.round(value));
		});
		expect(new Set(bounds.map((value) => value.join(","))).size).toBe(3);
		expect(
			geometries.every(({ triangles }) =>
				triangles.every(({ color }) => color.join(",") === "0.13,0.15,0.18"),
			),
		).toBe(true);
	});

	it("reduces a live 3D box to technical outline edges without face diagonals", () => {
		const p = {
			lbf: [-100, -100, 100],
			rbf: [100, -100, 100],
			rtf: [100, 100, 100],
			ltf: [-100, 100, 100],
			lbb: [-100, -100, -100],
			rbb: [100, -100, -100],
			rtb: [100, 100, -100],
			ltb: [-100, 100, -100],
		} as const;
		const face = (
			first: (typeof p)[keyof typeof p],
			second: (typeof p)[keyof typeof p],
			third: (typeof p)[keyof typeof p],
		) => ({
			pointsMillimetres: [first, second, third] as [
				[number, number, number],
				[number, number, number],
				[number, number, number],
			],
			colour: [0.8, 0.2, 0.2] as [number, number, number],
		});
		const triangles = [
			face(p.lbf, p.rbf, p.rtf),
			face(p.lbf, p.rtf, p.ltf),
			face(p.rbb, p.lbb, p.ltb),
			face(p.rbb, p.ltb, p.rtb),
			face(p.lbb, p.lbf, p.ltf),
			face(p.lbb, p.ltf, p.ltb),
			face(p.rbf, p.rbb, p.rtb),
			face(p.rbf, p.rtb, p.rtf),
			face(p.ltf, p.rtf, p.rtb),
			face(p.ltf, p.rtb, p.ltb),
			face(p.lbb, p.rbb, p.rbf),
			face(p.lbb, p.rbf, p.lbf),
		];
		const geometry = entityPlanGeometry(
			{ ...movingLight, rotationDegrees: [0, 0, 0] },
			{
				id: "technical-box",
				projections: [],
				liveMeshes: [
					{ pose: "top", triangles },
					{ pose: "elevation", triangles },
				],
			},
			"front_to_back",
		);

		expect(geometry.source).toBe("live_model");
		expect(geometry.triangles).toHaveLength(4);
		expect(geometry.lines).toHaveLength(4);
		expect(geometry.outlines).toHaveLength(0);
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

	it("uses longitudinal chord pipes and lattice braces for truss instead of a box", () => {
		const geometry = entityPlanGeometry(
			{
				...movingLight,
				name: "Four-Point Truss 4 m",
				kind: "venue",
				fixtureType: "truss",
				sizeMillimetres: [4000, 290, 290],
			},
			undefined,
			"top_down",
		);

		expect(geometry.source).toBe("typed");
		expect(geometry.outlines.length).toBeGreaterThan(4);
	});

	it("shows the declared three- and four-point chord arrangement in truss end views", () => {
		const endView = (name: string) =>
			entityPlanGeometry(
				{
					...movingLight,
					name,
					kind: "venue",
					fixtureType: "truss",
					sizeMillimetres: [4000, 290, 290],
				},
				undefined,
				"left_to_right",
			);

		const threePoint = endView("Three-Point Truss 4 m");
		const fourPoint = endView("Four-Point Truss 4 m");
		expect(threePoint.source).toBe("typed");
		expect(threePoint.outlines).toHaveLength(6);
		expect(fourPoint.outlines).toHaveLength(8);
	});

	it("prefers canonical generated truss geometry over its typed fallback", () => {
		const geometry = entityPlanGeometry(
			{
				...movingLight,
				name: "Four-Point Truss 4 m",
				kind: "venue",
				fixtureType: "truss",
				sizeMillimetres: [4000, 290, 290],
			},
			{
				id: "generated-truss",
				projections: [
					{
						view: "left",
						svg: '<svg><path d="M 0 0 L 50 0 L 25 50 Z" fill="#171b20" /><path d="M 3 3 L 47 3 L 25 46 Z" fill="#7a828d" /></svg>',
						viewBoxMillimetres: [0, 0, 50, 50],
						originMillimetres: [25, 25],
					},
				],
			},
			"left_to_right",
		);

		expect(geometry.source).toBe("model");
		expect(geometry.lines.length).toBeGreaterThan(0);
		expect(geometry.triangles.map(({ color }) => color)).toEqual([
			[23 / 255, 27 / 255, 32 / 255],
			[122 / 255, 130 / 255, 141 / 255],
		]);
	});

	it("uses repeated human plan marks for crowd areas instead of the model box", () => {
		const crowd = {
			...movingLight,
			name: "Dancefloor Crowd",
			kind: "venue",
			fixtureType: "crowd_area",
			sizeMillimetres: [8000, 4000, 1800] as [number, number, number],
		};
		const suppliedBox: CadDrawing = {
			id: "generated-crowd",
			projections: [
				{
					view: "top",
					svg: '<svg><path d="M 0 0 L 8000 0 L 8000 4000 L 0 4000 Z" fill="#66707a" /></svg>',
					viewBoxMillimetres: [0, 0, 8000, 4000],
					originMillimetres: [4000, 2000],
				},
			],
		};
		const top = entityPlanGeometry(crowd, suppliedBox, "top_down");
		const side = entityPlanGeometry(crowd, suppliedBox, "left_to_right");
		const oppositeSide = entityPlanGeometry(
			crowd,
			suppliedBox,
			"right_to_left",
		);
		const back = entityPlanGeometry(crowd, suppliedBox, "back_to_front");
		const front = entityPlanGeometry(crowd, suppliedBox, "front_to_back");

		expect(top.source).toBe("typed");
		expect(audienceOutline.source).toBe("assets/viz/crowd/Person Outline.svg");
		expect(audienceOutline.top_strokes).toHaveLength(5);
		expect(audienceOutline.front_strokes).toHaveLength(4);
		expect(audienceOutline.side_strokes).toHaveLength(5);
		expect(top.outlines).toHaveLength(24 * 5);
		expect(side.outlines).toHaveLength(8 * 5);
		expect(oppositeSide.outlines).toHaveLength(8 * 5);
		expect(back.outlines).toHaveLength(14 * 4);
		expect(front.outlines).toHaveLength(14 * 4);
		expect(top.outlines.slice(0, 5).map((outline) => outline.length)).toEqual(
			audienceOutline.top_strokes.map((stroke) => stroke.length),
		);
		expect(side.outlines.slice(0, 5).map((outline) => outline.length)).toEqual(
			audienceOutline.side_strokes.map((stroke) => stroke.length),
		);
		expect(front.outlines.slice(0, 4).map((outline) => outline.length)).toEqual(
			audienceOutline.front_strokes.map((stroke) => stroke.length),
		);
		const sideBodyMaxX = Math.max(
			...audienceOutline.side_strokes[1].map(([x]) => x),
		);
		const sideArmMaxX = Math.max(
			...audienceOutline.side_strokes[2].map(([x]) => x),
		);
		expect(sideArmMaxX).toBeGreaterThan(sideBodyMaxX);
		expect(top.triangles).toEqual([]);
		expect(side.outlines).not.toEqual(oppositeSide.outlines);
		expect(side.outlines).not.toEqual(back.outlines);
		expect(back.outlines).toEqual(front.outlines);
		const sideY = side.outlines.flat().map((point) => point[1]);
		expect(Math.min(...sideY)).toBeGreaterThan(-30);
		expect(Math.max(...sideY)).toBeGreaterThanOrEqual(1600);
		expect(Math.max(...sideY)).toBeLessThanOrEqual(1850);
		const heights = Array.from({ length: 24 }, (_, index) =>
			audiencePersonHeight(index),
		);
		expect(new Set(heights).size).toBeGreaterThan(12);
		expect(heights.every((height) => height >= 1600 && height <= 1850)).toBe(
			true,
		);
	});
});
