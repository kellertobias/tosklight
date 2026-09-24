import { describe, expect, it } from "vitest";
import { audienceOutline } from "./audienceOutline";
import {
	CHAIN_PITCH,
	CHAIN_WIRE,
	SHACKLE,
	STEELFLEX_LEG_DEGREES,
	STEELFLEX_WIDTH,
} from "./chainPlan";
import {
	audiencePersonHeight,
	audiencePersonScale,
	crowdGrid,
	entityPlanGeometry,
	SCISSOR_MAX_DEGREES,
	parseProjection,
	projectionViewForCad,
} from "./projection";
import { trussBays, trussParts } from "./trussPlan";
import type { CadDrawing, CadEntity, CadViewDirection } from "./types";

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
	it("draws a box, a cylinder and a ball filling their size in every view", () => {
		const shape = (kind: string, view: CadViewDirection) => {
			const geometry = entityPlanGeometry(
				{
					...movingLight,
					name: kind,
					kind: "venue",
					fixtureType: "venue",
					// Width, depth and height.
					sizeMillimetres: [2000, 1000, 3000],
					scenery: { kind, chords: 0, pattern: "standard" },
				},
				undefined,
				view,
			);
			const points = geometry.triangles.flatMap((triangle) => triangle.points);
			const extent = (axis: 0 | 1) => Math.max(...points.map((point) => Math.abs(point[axis])));
			return { corners: new Set(points.map((point) => point.join(","))).size, x: extent(0), y: extent(1) };
		};
		const close = (actual: { x: number; y: number }, x: number, y: number) => {
			expect(actual.x).toBeCloseTo(x, 0);
			expect(actual.y).toBeCloseTo(y, 0);
		};
		// From above the plan is width by depth; from the front width by height; from the side
		// depth by height.
		for (const [view, x, y] of [
			["top_down", 1000, 500],
			["front_to_back", 1000, 1500],
			["left_to_right", 500, 1500],
		] as const) {
			const box = shape("box", view);
			expect(box.corners).toBe(4);
			close(box, x, y);
			const ball = shape("sphere", view);
			expect(ball.corners).toBeGreaterThan(16);
			close(ball, x, y);
			// An upright cylinder is round from above and a rectangle from any side.
			const cylinder = shape("cylinder", view);
			expect(cylinder.corners).toBe(view === "top_down" ? 32 : 4);
			close(cylinder, x, y);
		}
	});

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
		// Braces run behind the chords, so most of a truss is drawn as the lines that show.
		expect(geometry.outlines.length + geometry.lines.length).toBeGreaterThan(4);
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
		// Looking down a truss, each chord end is its receiver ring with the coupler inside; the
		// receiver hides the chord behind it. The end frame links the receivers, and a box's frame
		// carries one diagonal across it, drawn where the receivers leave it showing.
		expect(threePoint.outlines).toHaveLength(3 * 2);
		expect(fourPoint.outlines).toHaveLength(4 * 2);
		const frame = (geometry: typeof threePoint) =>
			geometry.lines.filter(
				({ points: [[ax, ay], [bx, by]] }) => Math.hypot(bx - ax, by - ay) > 100,
			);
		expect(frame(threePoint)).toHaveLength(3 * 2);
		expect(frame(fourPoint)).toHaveLength(5 * 2);
	});

	it("draws a truss from its declared build: 45° bays, end frames, receivers and couplers", () => {
		const truss = (scenery: CadEntity["scenery"], view: CadViewDirection) =>
			entityPlanGeometry(
				{
					...movingLight,
					name: "SR boom",
					kind: "venue",
					fixtureType: "rigging",
					sizeMillimetres: [3000, 290, 290],
					scenery,
				},
				undefined,
				view,
			);
		const side = truss(
			{ kind: "truss", chords: 4, pattern: "standard" },
			"front_to_back",
		);
		const parts = trussParts(290, 4);
		const bays = trussBays(parts, 3000);
		// A node about every chord spacing keeps the diagonals near 45°, as F34 and H30V are built.
		expect(bays).toBe(12);
		const bay = (3000 - parts.endFrame * 2) / bays;
		expect((Math.atan(parts.spacing / bay) * 180) / Math.PI).toBeCloseTo(45, 0);
		// Near and far faces cross in every bay. The near diagonal hides the far one where it passes
		// behind, so every drawn diagonal runs at the bay's angle and each X shows six long edges.
		const diagonals = side.lines.filter(
			({ points: [[ax, ay], [bx, by]] }) => Math.abs(bx - ax) > 1 && Math.abs(by - ay) > 1,
		);
		const degrees = ({ points: [[ax, ay], [bx, by]] }: (typeof diagonals)[number]) =>
			(Math.atan2(Math.abs(by - ay), Math.abs(bx - ax)) * 180) / Math.PI;
		expect(diagonals.every((line) => Math.abs(degrees(line) - 45) < 2)).toBe(true);
		const long = diagonals.filter(
			({ points: [[ax, ay], [bx, by]] }) => Math.hypot(bx - ax, by - ay) > 40,
		);
		expect(long).toHaveLength(6 * bays);
		// Only the couplers, in front of everything at the four chord ends, stay whole.
		expect(side.outlines).toHaveLength(4);
		const couplers = side.triangles.filter(
			({ color }) => color.join() === "0.72,0.75,0.79",
		);
		expect(couplers.length).toBeGreaterThan(0);
		const xs = [...side.outlines.flat(), ...side.lines.flatMap(({ points }) => points)].map(
			([x]) => x,
		);
		expect(Math.min(...xs)).toBeLessThan(-1500);
		expect(Math.max(...xs)).toBeGreaterThan(1500);

		const drawn = (geometry: typeof side) => geometry.outlines.length + geometry.lines.length;
		const deco = truss({ kind: "truss", chords: 4, pattern: "deco" }, "top_down");
		const standardTop = truss(
			{ kind: "truss", chords: 4, pattern: "standard" },
			"top_down",
		);
		expect(drawn(deco)).toBe(drawn(standardTop));
		const ladderTop = truss(
			{ kind: "truss", chords: 2, pattern: "standard" },
			"top_down",
		);
		expect(drawn(ladderTop)).toBeLessThan(drawn(standardTop));
	});

	it("draws a curtain as a wave from above and a dotted-fold rectangle from the front", () => {
		const curtain = (view: CadViewDirection) =>
			entityPlanGeometry(
				{
					...movingLight,
					name: "Upstage black",
					kind: "venue",
					fixtureType: "venue",
					sizeMillimetres: [4500, 60, 6000],
					scenery: { kind: "curtain", chords: 0, pattern: "standard" },
				},
				undefined,
				view,
			);
		const top = curtain("top_down");
		expect(top.outlines).toHaveLength(0);
		const ys = top.lines.flatMap(({ points }) => points.map(([, y]) => y));
		// A sine wave across the track, half as deep as the curtain is gathered.
		expect(Math.max(...ys)).toBeCloseTo(15, 0);
		expect(Math.min(...ys)).toBeCloseTo(-15, 0);
		const xs = top.lines.flatMap(({ points }) => points.map(([x]) => x));
		expect(Math.min(...xs)).toBe(-2250);
		expect(Math.max(...xs)).toBe(2250);

		const front = curtain("front_to_back");
		expect(front.outlines).toEqual([
			[
				[-2250, -3000],
				[2250, -3000],
				[2250, 3000],
				[-2250, 3000],
			],
		]);
		// Each seam is a dotted line whose dashes lean 10–20° off vertical, alternating sides, and
		// every dash stays inside the rectangle.
		const leans = front.lines.map(({ points: [[ax, ay], [bx, by]] }) =>
			Math.round((Math.atan2(bx - ax, by - ay) * 180) / Math.PI),
		);
		expect(leans.every((lean) => Math.abs(lean) >= 10 && Math.abs(lean) <= 20)).toBe(true);
		expect(leans.some((lean) => lean > 0)).toBe(true);
		expect(leans.some((lean) => lean < 0)).toBe(true);
		const inside = front.lines.every(({ points }) =>
			points.every(([x, y]) => Math.abs(x) <= 2250 + 1e-6 && Math.abs(y) <= 3000 + 1e-6),
		);
		expect(inside).toBe(true);
		expect(front.lines.length).toBeGreaterThan(9 * 5);
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
		// 8 × 4 m holds 11 people across in 6 rows, at a standing audience's spacing.
		expect(crowdGrid(8000, 4000)).toEqual({ columns: 11, rows: 6 });
		expect(top.outlines).toHaveLength(66 * 5);
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
		expect(Math.max(...sideY)).toBeLessThanOrEqual(1850 + 1);
		// Real audiences: 1.55 to 1.85 m, and nobody drawn two metres tall.
		const heights = Array.from({ length: 200 }, (_, index) =>
			audiencePersonHeight(index, 7),
		);
		expect(new Set(heights).size).toBeGreaterThan(150);
		expect(heights.every((height) => height >= 1550 && height <= 1850)).toBe(true);
		expect(Math.max(...heights)).toBeGreaterThan(1800);
		expect(Math.min(...heights)).toBeLessThan(1600);
	});

	it("fills a larger footprint with more people rather than stretching the same ones", () => {
		const plan = (width: number, depth: number) =>
			entityPlanGeometry(
				{
					...movingLight,
					name: "Dancefloor Crowd",
					kind: "venue",
					fixtureType: "crowd_area",
					sizeMillimetres: [width, depth, 1780],
				},
				undefined,
				"top_down",
			).outlines;
		const people = (width: number, depth: number) => plan(width, depth).length / 5;
		expect(people(5000, 3000)).toBe(7 * 4);
		expect(people(10_000, 3000)).toBe(14 * 4);
		expect(people(5000, 6000)).toBe(7 * 9);
		// Every person stays inside the footprint, however it is sized.
		for (const [width, depth] of [
			[5000, 3000],
			[1000, 1000],
			[20_000, 12_000],
		]) {
			const points = plan(width, depth).flat();
			const furthest = (axis: 0 | 1) =>
				points.reduce((most, point) => Math.max(most, Math.abs(point[axis])), 0);
			expect(furthest(0)).toBeLessThanOrEqual(width / 2);
			expect(furthest(1)).toBeLessThanOrEqual(depth / 2);
		}
		// The side view lines up more figures along a wider crowd too.
		const side = (width: number) =>
			entityPlanGeometry(
				{
					...movingLight,
					name: "Dancefloor Crowd",
					kind: "venue",
					fixtureType: "crowd_area",
					sizeMillimetres: [width, 3000, 1780],
				},
				undefined,
				"front_to_back",
			).outlines.length;
		expect(side(6000)).toBeGreaterThan(side(3000));
	});

	it("sizes every person in a crowd differently, yet the same on every redraw", () => {
		const crowd = (id: string, view: CadViewDirection) =>
			entityPlanGeometry(
				{
					...movingLight,
					id,
					name: "Dancefloor Crowd",
					kind: "venue",
					fixtureType: "crowd_area",
					sizeMillimetres: [8000, 4000, 1800],
				},
				undefined,
				view,
			);
		const people = (outlines: [number, number][][], strokes: number) =>
			Array.from({ length: outlines.length / strokes }, (_, person) => {
				const points = outlines.slice(person * strokes, (person + 1) * strokes).flat();
				const xs = points.map(([x]) => x);
				const ys = points.map(([, y]) => y);
				return [Math.max(...xs) - Math.min(...xs), Math.max(...ys) - Math.min(...ys)];
			});
		for (const [view, strokes] of [
			["top_down", 5],
			["front_to_back", 4],
			["left_to_right", 5],
		] as const) {
			const first = crowd(movingLight.id, view);
			expect(crowd(movingLight.id, view)).toEqual(first);
			const sizes = people(first.outlines, strokes);
			const widths = sizes.map(([width]) => width);
			const heights = sizes.map(([, height]) => height);
			// Roughly ±15 %: the tallest and widest are clearly bigger, never absurdly so.
			expect(Math.max(...heights) / Math.min(...heights)).toBeGreaterThan(1.1);
			expect(Math.max(...heights) / Math.min(...heights)).toBeLessThan(1.36);
			expect(Math.max(...widths) / Math.min(...widths)).toBeGreaterThan(1.1);
			expect(crowd("22222222-2222-4222-8222-222222222222", view).outlines).not.toEqual(
				first.outlines,
			);
		}
		const scales = Array.from({ length: 200 }, (_, index) => audiencePersonScale(index, 7));
		expect(
			scales.every(
				({ width, height }) =>
					width >= 0.85 && width <= 1.15 && height >= 1550 / 1700 && height <= 1850 / 1700,
			),
		).toBe(true);
	});

	const chain = (
		view: CadViewDirection,
		chainMode?: "plain" | "motor_top" | "motor_bottom",
		height = 3000,
		anchor?: "steelflex" | "flange" | "shackle",
	) =>
		entityPlanGeometry(
			{
				...movingLight,
				name: "Chain",
				kind: "venue",
				fixtureType: "rigging",
				sizeMillimetres: [300, 300, height],
				scenery: { kind: "chain", chords: 0, pattern: "standard", chain: chainMode, anchor },
			},
			undefined,
			view,
		);
	const extent = (outline: [number, number][]) => {
		const xs = outline.map(([x]) => x);
		const ys = outline.map(([, y]) => y);
		return {
			width: Math.max(...xs) - Math.min(...xs),
			height: Math.max(...ys) - Math.min(...ys),
			centre: (Math.max(...ys) + Math.min(...ys)) / 2,
		};
	};
	const isStadium = (outline: [number, number][]) => {
		// Every point sits on the rounded rectangle whose corner radius is half its short side.
		const { width, height } = extent(outline);
		const xs = outline.map(([x]) => x);
		const ys = outline.map(([, y]) => y);
		const cx = (Math.max(...xs) + Math.min(...xs)) / 2;
		const cy = (Math.max(...ys) + Math.min(...ys)) / 2;
		const r = Math.min(width, height) / 2;
		return outline.every(([x, y]) => {
			const dx = Math.max(0, Math.abs(x - cx) - (width / 2 - r));
			const dy = Math.max(0, Math.abs(y - cy) - (height / 2 - r));
			return Math.abs(Math.hypot(dx, dy) - r) < 0.01;
		});
	};

	const samples = (lines: { points: [[number, number], [number, number]] }[]) =>
		lines.flatMap(({ points: [[ax, ay], [bx, by]] }) =>
			[0, 0.25, 0.5, 0.75, 1].map(
				(t) => [ax + (bx - ax) * t, ay + (by - ay) * t] as [number, number],
			),
		);

	it("draws a chain from above as its two links crossed, the upper hiding the lower", () => {
		for (const mode of ["plain", "motor_bottom"] as const) {
			const top = chain("top_down", mode);
			// The top link runs across and stays whole; the link through it runs along, beneath.
			expect(top.outlines).toHaveLength(1);
			expect(isStadium(top.outlines[0])).toBe(true);
			const { width, height } = extent(top.outlines[0]);
			expect([Math.round(width), Math.round(height)]).toEqual([24, 7]);
			const lower = samples(top.lines);
			expect(Math.max(...lower.map(([, y]) => y))).toBeCloseTo(12, 6);
			expect(Math.min(...lower.map(([, y]) => y))).toBeCloseTo(-12, 6);
			// Nothing of the lower link is drawn where the upper one lies over it.
			expect(
				lower.some(([x, y]) => Math.abs(x) < 12 - 1e-6 && Math.abs(y) < CHAIN_WIRE / 2 - 1e-6),
			).toBe(false);
			expect(top.triangles.length).toBeGreaterThan(0);
		}
	});

	it("hides the chain under a hoist on top, which is also what a chain without a mode is", () => {
		for (const top of [chain("top_down", "motor_top"), chain("top_down", undefined)]) {
			expect(top.outlines).toHaveLength(1);
			const { width, height } = extent(top.outlines[0]);
			expect([Math.round(width), Math.round(height)]).toEqual([280, 260]);
			expect(top.triangles.length).toBeGreaterThan(0);
		}
		expect(chain("front_to_back", undefined)).toEqual(chain("front_to_back", "motor_top"));
	});

	it("alternates face-on and edge-on links 21 mm apart, hiding the end wires the edge-on links cross", () => {
		expect(CHAIN_PITCH).toBe(21);
		const count = Math.floor((1000 - 35) / 21) + 1;
		const first = (35 + (count - 1) * 21) / 2 - 35 / 2;
		const centreOf = (index: number) => first - index * 21;
		for (const [view, faceParity] of [
			["front_to_back", 0],
			["left_to_right", 1],
		] as const) {
			const geometry = chain(view, "plain", 1000);
			// Edge-on links pass in front where links cross, so each stays a whole 7 × 35 stadium.
			const edgeOn = Array.from({ length: count }, (_, index) => index).filter(
				(index) => index % 2 !== faceParity,
			);
			expect(geometry.outlines.every(isStadium)).toBe(true);
			const shapes = geometry.outlines.map(extent);
			expect(shapes.map(({ width, height }) => [Math.round(width), Math.round(height)])).toEqual(
				edgeOn.map(() => [CHAIN_WIRE, 35]),
			);
			edgeOn.forEach((index, at) => expect(shapes[at].centre).toBeCloseTo(centreOf(index), 6));
			// A face-on link shows its outside and its opening at the sides…
			const drawn = samples(geometry.lines);
			for (let index = faceParity; index < count; index += 2) {
				const y = centreOf(index);
				const drawnAt = (x: number) =>
					drawn.some(([px, py]) => Math.abs(px - x) < 1e-6 && Math.abs(py - y) < 1e-6);
				expect(drawnAt(12)).toBe(true);
				expect(drawnAt(-5)).toBe(true);
				// …but not its end wires where an edge-on link passes in front of them.
				if (index > 0 && index < count - 1)
					expect(
						drawn.some(
							([px, py]) =>
								Math.abs(px) < CHAIN_WIRE / 2 - 1e-6 &&
								Math.abs(py - y) > 10.5 - 1e-6 &&
								Math.abs(py - y) < 17.5 + 1e-6,
						),
					).toBe(false);
			}
		}
	});

	it("fixes the chain's free end by what it hangs from, in a shackle whose bolt the last link bears on", () => {
		const slantDegrees = ({ points: [[ax, ay], [bx, by]] }: { points: [[number, number], [number, number]] }) =>
			(Math.atan2(Math.abs(bx - ax), Math.abs(by - ay)) * 180) / Math.PI;
		for (const [mode, sign] of [
			["motor_top", 1],
			["motor_bottom", -1],
		] as const)
			for (const anchor of ["steelflex", "flange", "shackle"] as const) {
				const front = chain("front_to_back", mode, 3000, anchor);
				const shapes = front.outlines.map(extent);
				// The bolt's head stands proud of the shackle's wall: 8 × 28.
				const head = shapes.find(
					({ width, height }) => Math.round(width) === 8 && Math.round(height) === 28,
				);
				expect(head).toBeDefined();
				// The last link hangs edge-on on the bolt, its end wire bearing on it.
				const links = shapes.filter(
					({ width, height }) => Math.round(width) === CHAIN_WIRE && Math.round(height) === 35,
				);
				const last = links.reduce((best, link) =>
					link.centre * sign < best.centre * sign ? link : best,
				);
				// Its end wire's inside meets the bolt's underside, so its centre is this far past the bolt.
				expect((last.centre - head!.centre) * -sign).toBeCloseTo(
					CHAIN_WIRE + SHACKLE.bolt / 2 - 35 / 2,
					6,
				);
				expect(sign * head!.centre).toBeLessThan(0);
				// Seen from the side, the bolt ends in its nut at the same height.
				const nut = chain("left_to_right", mode, 3000, anchor).outlines.find(
					(outline) => outline.length === 6,
				);
				expect(extent(nut!).centre).toBeCloseTo(head!.centre, 6);

				const legs = front.lines.filter(
					({ points: [[ax, ay], [bx, by]] }) =>
						Math.abs(bx - ax) > 1 && Math.abs(by - ay) > 1 && Math.hypot(bx - ax, by - ay) > 40,
				);
				const round = (size: number) =>
					shapes.some(({ width, height }) => Math.round(width) === size && Math.round(height) === size);
				if (anchor === "steelflex") {
					// Two legs 45° apart, each a sling 22 mm wide, round a 50 mm truss chord.
					expect(legs.length).toBeGreaterThanOrEqual(4);
					expect(legs.every((leg) => Math.abs(slantDegrees(leg) - STEELFLEX_LEG_DEGREES) < 1e-3)).toBe(true);
					const left = legs.filter(({ points }) => points[0][0] + points[1][0] < 0);
					const [[ax, ay], [bx, by]] = left[0].points;
					const length = Math.hypot(bx - ax, by - ay);
					const apart = Math.max(
						...left.map(({ points: [[px, py]] }) => Math.abs((bx - ax) * (py - ay) - (by - ay) * (px - ax)) / length),
					);
					expect(apart).toBeCloseTo(STEELFLEX_WIDTH, 3);
					expect(round(50)).toBe(true);
				} else {
					expect(legs).toHaveLength(0);
					expect(round(50)).toBe(false);
				}
				// A flange's clamp rings the pipe; nothing else has one.
				expect(round(64)).toBe(anchor === "flange");
				// The hoist is at the other end.
				const body = shapes.find(({ height }) => Math.round(height) === 420);
				expect(Math.sign(body!.centre)).toBe(sign);
			}
		// A plain chain draws no fixing and no hoist: nothing longer than one link.
		expect(
			chain("front_to_back", "plain").lines.every(
				({ points: [[ax, ay], [bx, by]] }) => Math.hypot(bx - ax, by - ay) <= 35,
			),
		).toBe(true);
	});

	it("leaves the opening of a face-on chain link hollow", () => {
		const geometry = chain("front_to_back", "plain", 500);
		const covered = ([x, y]: [number, number]) =>
			geometry.triangles.some(({ points: [a, b, c] }) => {
				const side = (p: [number, number], q: [number, number]) =>
					(q[0] - p[0]) * (y - p[1]) - (q[1] - p[1]) * (x - p[0]);
				const [ab, bc, ca] = [side(a, b), side(b, c), side(c, a)];
				return (ab >= 0 && bc >= 0 && ca >= 0) || (ab <= 0 && bc <= 0 && ca <= 0);
			});
		const count = Math.floor((500 - 35) / 21) + 1;
		const first = (35 + (count - 1) * 21) / 2 - 35 / 2;
		expect(count).toBeGreaterThan(10);
		for (let index = 0; index < count; index += 2) {
			const centre: [number, number] = [0, first - index * 21];
			expect(covered(centre)).toBe(false);
			// The wire at the side of the opening is still solid.
			expect(covered([12 - CHAIN_WIRE / 2, centre[1]])).toBe(true);
		}
	});

	it("lifts a stage element on scissor arms and leaves stairs on their legs", () => {
		const riser = (name: string, view: CadViewDirection, height: number, fixtureType = "venue") =>
			entityPlanGeometry(
				{
					...movingLight,
					name,
					kind: "venue",
					fixtureType,
					sizeMillimetres: [2000, 1000, height],
					scenery: { kind: "riser", chords: 0, pattern: "standard" },
				},
				undefined,
				view,
			);
		// Every arm is slanted; the deck, the base frame and the stairs are square to the page. Arms
		// cross and end inside the deck and the base, so they are drawn only as their visible lines.
		type Geometry = ReturnType<typeof riser>;
		const slanted = (geometry: Geometry) =>
			geometry.lines.filter(
				({ points: [[ax, ay], [bx, by]] }) => Math.abs(by - ay) > 1 && Math.abs(bx - ax) > 1,
			);
		const pivots = (geometry: Geometry) =>
			geometry.outlines.filter((outline) => outline.length === 10);
		const low = riser("Stage element 2x1", "front_to_back", 200);
		const tallFront = riser("Stage element 2x1", "front_to_back", 1200);
		const tallSide = riser("Stage element 2x1", "left_to_right", 1200);
		expect(pivots(low)).toHaveLength(1);
		expect(pivots(tallFront)).toHaveLength(1);
		// From the side the deck is only 1 m deep, so the same rise needs a second X.
		expect(pivots(tallSide)).toHaveLength(2);
		for (const [geometry, width, height, stages] of [
			[low, 2000, 200, 1],
			[tallFront, 2000, 1200, 1],
			[tallSide, 1000, 1200, 2],
		] as const) {
			const arms = slanted(geometry);
			const angle = ({ points: [[ax, ay], [bx, by]] }: (typeof arms)[number]) =>
				(Math.atan2(Math.abs(by - ay), Math.abs(bx - ax)) * 180) / Math.PI;
			
			// Where two arms cross, the front one hides the one behind: its two long edges are each
			// cut in two, and the front arm's stay whole — six long lines to an X.
			const long = arms.filter(
				({ points: [[ax, ay], [bx, by]] }) => Math.hypot(bx - ax, by - ay) > width * 0.2 / stages,
			);
			// The long edges run at the arm's angle; only the arms' cut ends, a few millimetres long, are steeper.
			expect(long.every((arm) => angle(arm) <= SCISSOR_MAX_DEGREES + 0.5)).toBe(true);
			if (stages === 1) expect(long).toHaveLength(6);
			else expect(long.length).toBeGreaterThanOrEqual(6 * stages);
			const points = arms.flatMap(({ points }) => points);
			const xs = points.map(([x]) => x);
			const ys = points.map(([, y]) => y);
			// The arms span nearly the whole deck and stop where the deck and the base frame begin.
			expect(Math.max(...xs) - Math.min(...xs)).toBeGreaterThan(width * 0.92);
			expect(Math.max(...xs)).toBeLessThanOrEqual(width / 2 + 1e-6);
			const deck = Math.min(80, height * 0.3);
			const base = Math.min(50, height * 0.2);
			const top = height - deck;
			const bottom = base;
			expect(Math.max(...ys)).toBeCloseTo(top, 3);
			expect(Math.min(...ys)).toBeCloseTo(bottom, 3);
			// The deck and the base frame are drawn whole, and nothing of an arm is drawn along
			// their edges or inside them.
			const rects = geometry.outlines.filter((outline) => outline.length === 4).map(extent);
			expect(rects.map(({ width: w, height: h }) => [Math.round(w), Math.round(h)])).toEqual([
				[width, Math.round(deck)],
				[width, Math.round(base)],
			]);
			expect(
				samples(geometry.lines).some(([, y]) => y > top + 1e-3 || y < bottom - 1e-3),
			).toBe(false);
			expect(
				geometry.lines.some(({ points }) =>
					points.every(([, y]) => Math.abs(y - top) < 1e-3 || Math.abs(y - bottom) < 1e-3),
				),
			).toBe(false);
		}
		const plan = riser("Stage element 2x1", "top_down", 1200);
		expect(slanted(plan)).toHaveLength(0);
		expect(plan.outlines).toHaveLength(2);

		const stairs = riser("Stage Stairs", "front_to_back", 1200);
		expect(slanted(stairs)).toHaveLength(0);
		expect(pivots(stairs)).toHaveLength(0);
		expect(slanted(riser("Treppe", "left_to_right", 1200, "stage_stairs"))).toHaveLength(0);
	});

	it("stands a deck on regular feet on one leg under each corner", () => {
		const deck = (view: CadViewDirection, height: number) =>
			entityPlanGeometry(
				{
					...movingLight,
					name: "Stage Deck 2 × 1 m",
					kind: "venue",
					fixtureType: "venue",
					sizeMillimetres: [2000, 1000, height],
					scenery: { kind: "riser", chords: 0, pattern: "standard", feet: "fixed" },
				},
				undefined,
				view,
			);
		const front = deck("front_to_back", 600);
		// Nothing is slanted: regular feet are legs, not scissor arms, and there is no pivot.
		expect(
			front.lines.filter(
				({ points: [[ax, ay], [bx, by]] }) => Math.abs(by - ay) > 1 && Math.abs(bx - ax) > 1,
			),
		).toHaveLength(0);
		expect(front.outlines.filter((outline) => outline.length === 10)).toHaveLength(0);
		// The top is drawn whole, its surface at the placed height, 40 mm thick over the legs.
		expect(front.outlines).toHaveLength(1);
		const top = extent(front.outlines[0]);
		expect([Math.round(top.width), Math.round(top.height)]).toEqual([2000, 40]);
		expect(top.centre).toBeCloseTo(580, 3);
		// A leg under each corner carries it from the floor, its outer face flush with the deck's
		// edge. Each leg's own top edge is under the deck, so only its other three are drawn.
		const legEdges = front.lines.flatMap(({ points }) => points);
		expect(Math.min(...legEdges.map(([, y]) => y))).toBeCloseTo(0, 3);
		expect(Math.max(...legEdges.map(([, y]) => y))).toBeCloseTo(560, 3);
		expect([...new Set(legEdges.map(([x]) => Math.round(x)))].sort((a, b) => a - b)).toEqual([
			-1000, -940, 940, 1000,
		]);
		// Seen from above a deck is its platform, the same as any other stage element.
		expect(deck("top_down", 600).outlines).toHaveLength(2);
	});

	it("keeps drawing a withdrawn deck on fixed legs from a show that still holds one", () => {
		// The fifteen decks on fixed legs were withdrawn from the library once the three generated
		// ones could be built to any height. A show that patched one carries its own copy of that
		// profile, which has no scenery at all, so its name is the only thing that says what it is.
		const legged = entityPlanGeometry(
			{
				...movingLight,
				name: "Stage Deck 2 × 1 m, Legs 0.4 m",
				fixtureProfile: "Venue Stage Deck 2 × 1 m, Legs 0.4 m",
				kind: "venue",
				fixtureType: "venue",
				sizeMillimetres: [2000, 1000, 440],
			} as CadEntity,
			undefined,
			"front_to_back",
		);
		// It still stands on its feet and still draws its legs, not a scissor lift.
		const ys = [...legged.outlines.flat(), ...legged.lines.flatMap(({ points }) => points)].map(
			([, y]) => y,
		);
		expect(Math.min(...ys)).toBeCloseTo(0, 3);
		expect(Math.max(...ys)).toBeCloseTo(440, 3);
		expect(
			legged.lines.filter(
				({ points: [[ax, ay], [bx, by]] }) => Math.abs(by - ay) > 1 && Math.abs(bx - ax) > 1,
			),
		).toHaveLength(0);
	});

	it("builds a flight of stairs from the kind its profile declares, handrails and all", () => {
		const flight = (handrails: boolean, view: CadViewDirection = "front_to_back") =>
			entityPlanGeometry(
				{
					...movingLight,
					name: "Stage Stairs",
					kind: "venue",
					fixtureType: "venue",
					sizeMillimetres: [1000, 2800, 600],
					scenery: { kind: "stairs", chords: 0, pattern: "standard", handrails },
				},
				undefined,
				view,
			);
		const heights = (geometry: ReturnType<typeof flight>) =>
			[
				...geometry.triangles.flatMap(({ points }) => points),
				...geometry.outlines.flat(),
				...geometry.lines.flatMap(({ points }) => points),
			].map(([, y]) => y);
		// Three 200 mm rises to a 600 mm deck: the flight stands on the floor and meets the deck.
		const plain = flight(false);
		expect(Math.min(...heights(plain))).toBeCloseTo(0, 3);
		expect(Math.max(...heights(plain))).toBeCloseTo(600, 3);
		// Each step is a block from the floor to its own nosing, so a nosing sits at every rise.
		const nosings = [...new Set(heights(plain).map((y) => Math.round(y)))].sort((a, b) => a - b);
		expect(nosings).toEqual([0, 200, 400, 600]);
		// A rail post stands on every nosing and the rail follows the climb above them.
		const railed = flight(true);
		expect(Math.max(...heights(railed))).toBeCloseTo(1200, 3);
		expect(railed.lines.length).toBeGreaterThan(plain.lines.length);
	});

	it("draws a handrail as posts under a top rail and a knee rail, standing on its own feet", () => {
		const rail = (view: CadViewDirection) =>
			entityPlanGeometry(
				{
					...movingLight,
					name: "Stage Handrail",
					kind: "venue",
					fixtureType: "venue",
					sizeMillimetres: [4000, 40, 1000],
					scenery: { kind: "railing", chords: 0, pattern: "standard" },
				},
				undefined,
				view,
			);
		const front = rail("front_to_back");
		const ys = [
			...front.triangles.flatMap(({ points }) => points),
			...front.outlines.flat(),
			...front.lines.flatMap(({ points }) => points),
		].map(([, y]) => y);
		// It stands on the floor it is placed on and reaches its own height, like a stage element.
		expect(Math.min(...ys)).toBeCloseTo(0, 3);
		expect(Math.max(...ys)).toBeCloseTo(1000, 3);
		// Two rails run the whole length: a top rail and a knee rail.
		const bands = front.outlines.map(extent).filter(({ width }) => Math.round(width) === 4000);
		expect(bands).toHaveLength(2);
		// Seen from above it is the thin line of its own run.
		const plan = extent(rail("top_down").outlines[0]);
		expect([Math.round(plan.width), Math.round(plan.height)]).toEqual([4000, 40]);
	});

	it("stands a stage element and stairs on their origin and raises the deck with the height", () => {
		const elevation = (name: string, view: CadViewDirection, height: number) => {
			const geometry = entityPlanGeometry(
				{
					...movingLight,
					name,
					kind: "venue",
					fixtureType: "venue",
					sizeMillimetres: [2000, 1000, height],
					scenery: { kind: "riser", chords: 0, pattern: "standard" },
				},
				undefined,
				view,
			);
			const ys = [
				...geometry.triangles.flatMap(({ points }) => points),
				...geometry.outlines.flat(),
				...geometry.lines.flatMap(({ points }) => points),
			].map(([, y]) => y);
			return { bottom: Math.min(...ys), top: Math.max(...ys) };
		};
		for (const name of ["Stage element 2x1", "Stage Stairs"])
			for (const view of ["front_to_back", "back_to_front", "left_to_right", "right_to_left"] as const) {
				const low = elevation(name, view, 300);
				const high = elevation(name, view, 1100);
				// The feet stay on the origin; only the deck moves.
				expect(low.bottom, `${name} ${view}`).toBeCloseTo(0, 3);
				expect(high.bottom, `${name} ${view}`).toBeCloseTo(0, 3);
				// A flight of stairs climbs to the height it is placed at, as a deck's surface
				// does, so a flight set to a deck's height meets that deck.
				expect(low.top, `${name} ${view}`).toBeCloseTo(300, 3);
				expect(high.top, `${name} ${view}`).toBeCloseTo(1100, 3);
			}
		// From above, the footprint stays centred on the origin.
		const plan = elevation("Stage element 2x1", "top_down", 1100);
		expect(plan.bottom).toBeCloseTo(-500, 3);
		expect(plan.top).toBeCloseTo(500, 3);
	});
});
