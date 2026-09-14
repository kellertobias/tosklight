import { describe, expect, it } from "vitest";
import { audienceOutline } from "./audienceOutline";
import {
	CHAIN_PITCH,
	CHAIN_WIRE,
	STEELFLEX_LEG_DEGREES,
} from "./chainPlan";
import {
	audiencePersonHeight,
	audiencePersonScale,
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
		// Each chord shows its receiver, chord and coupler rings; the end frame links them, and a
		// box's frame carries one diagonal across it.
		expect(threePoint.outlines).toHaveLength(3 * 3 + 3);
		expect(fourPoint.outlines).toHaveLength(4 * 3 + 5);
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
		// Near and far faces cross in every bay, two end frames, two chords with two receivers and
		// two couplers each.
		expect(side.outlines).toHaveLength(2 * bays + 2 + 2 + 4 + 4);
		const couplers = side.triangles.filter(
			({ color }) => color.join() === "0.72,0.75,0.79",
		);
		expect(couplers.length).toBeGreaterThan(0);
		const xs = side.outlines.flat().map(([x]) => x);
		expect(Math.min(...xs)).toBeLessThan(-1500);
		expect(Math.max(...xs)).toBeGreaterThan(1500);

		const deco = truss({ kind: "truss", chords: 4, pattern: "deco" }, "top_down");
		const standardTop = truss(
			{ kind: "truss", chords: 4, pattern: "standard" },
			"top_down",
		);
		expect(deco.outlines.length).toBe(standardTop.outlines.length);
		const ladderTop = truss(
			{ kind: "truss", chords: 2, pattern: "standard" },
			"top_down",
		);
		expect(ladderTop.outlines.length).toBeLessThan(standardTop.outlines.length);
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
		expect(Math.max(...sideY)).toBeLessThanOrEqual(1750 * 1.15 + 1);
		const heights = Array.from({ length: 24 }, (_, index) =>
			audiencePersonHeight(index),
		);
		expect(new Set(heights).size).toBeGreaterThan(12);
		expect(
			heights.every((height) => height >= 1750 * 0.85 && height <= 1750 * 1.15),
		).toBe(true);
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
					width >= 0.85 && width <= 1.15 && height >= 0.85 && height <= 1.15,
			),
		).toBe(true);
	});

	const chain = (
		view: CadViewDirection,
		chainMode?: "plain" | "motor_top" | "motor_bottom",
		height = 3000,
	) =>
		entityPlanGeometry(
			{
				...movingLight,
				name: "Chain",
				kind: "venue",
				fixtureType: "rigging",
				sizeMillimetres: [300, 300, height],
				scenery: { kind: "chain", chords: 0, pattern: "standard", chain: chainMode },
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

	it("draws a chain from above as its two link orientations crossed", () => {
		for (const mode of ["plain", "motor_bottom"] as const) {
			const top = chain("top_down", mode);
			expect(top.outlines).toHaveLength(2);
			expect(top.outlines.every(isStadium)).toBe(true);
			const sizes = top.outlines.map(extent).map(({ width, height }) => [
				Math.round(width),
				Math.round(height),
			]);
			expect(sizes).toEqual([
				[24, 7],
				[7, 24],
			]);
			expect(top.lines).toHaveLength(0);
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

	it("alternates face-on and edge-on links 21 mm apart, swapped between front and side", () => {
		const plain = chain("front_to_back", "plain", 1000);
		const side = chain("left_to_right", "plain", 1000);
		expect(CHAIN_PITCH).toBe(21);
		const links = (geometry: typeof plain) => {
			const shapes = geometry.outlines.map(extent);
			const found: { kind: "face" | "edge"; centre: number }[] = [];
			for (let index = 0; index < shapes.length; index++) {
				const shape = shapes[index];
				expect(isStadium(geometry.outlines[index])).toBe(true);
				if (Math.round(shape.width) === 24) {
					expect(Math.round(shape.height)).toBe(35);
					const opening = shapes[index + 1];
					expect([Math.round(opening.width), Math.round(opening.height)]).toEqual([10, 21]);
					expect(opening.centre).toBeCloseTo(shape.centre, 6);
					found.push({ kind: "face", centre: shape.centre });
					index++;
				} else {
					expect([Math.round(shape.width), Math.round(shape.height)]).toEqual([CHAIN_WIRE, 35]);
					found.push({ kind: "edge", centre: shape.centre });
				}
			}
			return found;
		};
		const front = links(plain);
		const sideLinks = links(side);
		expect(front.length).toBe(Math.floor((1000 - 35) / 21) + 1);
		front.forEach((link, index) => {
			expect(link.kind).toBe(index % 2 ? "edge" : "face");
			expect(sideLinks[index].kind).toBe(index % 2 ? "face" : "edge");
			if (index) expect(front[index - 1].centre - link.centre).toBeCloseTo(21, 6);
		});
		// An edge-on link reaches a wire's depth into the face-on link's opening at each end.
		const edgeTop = front[1].centre + 35 / 2;
		const edgeBottom = front[1].centre - 35 / 2;
		expect(edgeTop - (front[0].centre - 21 / 2)).toBeCloseTo(CHAIN_WIRE, 6);
		expect(front[2].centre + 21 / 2 - edgeBottom).toBeCloseTo(CHAIN_WIRE, 6);
		expect(plain.lines).toHaveLength(0);
	});

	it("shackles the chain end to a steelflex whose legs meet at 45° from a wrap round the chord", () => {
		for (const [mode, sign] of [
			["motor_top", 1],
			["motor_bottom", -1],
		] as const)
			for (const view of ["front_to_back", "left_to_right"] as const) {
				const geometry = chain(view, mode);
				const legs = geometry.lines.filter(({ points: [[ax, ay], [bx, by]] }) => {
					const length = Math.hypot(bx - ax, by - ay);
					return length > 20 && Math.abs(Math.abs(ax) - Math.abs(bx)) > 10;
				});
				expect(legs).toHaveLength(2);
				for (const {
					points: [[ax, ay], [bx, by]],
				} of legs) {
					const degrees = (Math.atan2(Math.abs(bx - ax), Math.abs(by - ay)) * 180) / Math.PI;
					expect(degrees).toBeCloseTo(STEELFLEX_LEG_DEGREES, 6);
					// The legs sit at the end away from the hoist: the bottom for a hoist on top.
					expect(Math.sign(ay + by)).toBe(-sign);
				}
				const apexes = legs.map(({ points }) => points[1]);
				expect(apexes[0]).toEqual(apexes[1]);
				// The chord the steelflex wraps is a 50 mm circle at the very end of the object.
				const chord = geometry.outlines
					.map(extent)
					.find(({ width, height }) => Math.round(width) === 50 && Math.round(height) === 50);
				expect(chord).toBeDefined();
				expect(Math.abs(chord!.centre)).toBeCloseTo(1500 - 28, 0);
				// The hoist is at the other end.
				const body = geometry.outlines
					.map(extent)
					.find(({ height }) => Math.round(height) === 420);
				expect(Math.sign(body!.centre)).toBe(sign);
			}
		expect(chain("front_to_back", "plain").lines).toHaveLength(0);
	});

	it("leaves the opening of a face-on chain link hollow", () => {
		const geometry = entityPlanGeometry(
			{
				...movingLight,
				name: "Chain",
				kind: "venue",
				fixtureType: "rigging",
				sizeMillimetres: [100, 100, 500],
				scenery: { kind: "chain", chords: 0, pattern: "standard", chain: "plain" },
			},
			undefined,
			"front_to_back",
		);
		const covered = ([x, y]: [number, number]) =>
			geometry.triangles.some(({ points: [a, b, c] }) => {
				const side = (p: [number, number], q: [number, number]) =>
					(q[0] - p[0]) * (y - p[1]) - (q[1] - p[1]) * (x - p[0]);
				const [ab, bc, ca] = [side(a, b), side(b, c), side(c, a)];
				return (ab >= 0 && bc >= 0 && ca >= 0) || (ab <= 0 && bc <= 0 && ca <= 0);
			});
		const faceOn = geometry.outlines
			.map((outline) => {
				const xs = outline.map(([x]) => x);
				const ys = outline.map(([, y]) => y);
				return {
					width: Math.max(...xs) - Math.min(...xs),
					height: Math.max(...ys) - Math.min(...ys),
					centre: [(Math.max(...xs) + Math.min(...xs)) / 2, (Math.max(...ys) + Math.min(...ys)) / 2] as [number, number],
				};
			})
			.filter(({ width, height }) => Math.round(width) === 24 && Math.round(height) === 35);
		expect(faceOn.length).toBeGreaterThan(5);
		for (const { centre } of faceOn) {
			expect(covered(centre)).toBe(false);
			// The wire at the side of the opening is still solid.
			expect(covered([centre[0] + 12 - CHAIN_WIRE / 2, centre[1]])).toBe(true);
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
		// An arm is the only slanted straight-sided shape: a clipped band of at most six corners with
		// an edge that runs neither across nor up. The round pivot has more corners.
		const arms = (geometry: ReturnType<typeof riser>) =>
			geometry.outlines.filter((outline) =>
				outline.length <= 6 &&
				outline.some(([ax, ay], index) => {
					const [bx, by] = outline[(index + 1) % outline.length];
					return Math.abs(by - ay) > 1 && Math.abs(bx - ax) > 1;
				}),
			);
		const angle = (outline: [number, number][]) => {
			const edges = outline.map(([ax, ay], index) => {
				const [bx, by] = outline[(index + 1) % outline.length];
				return [bx - ax, by - ay];
			});
			const [dx, dy] = edges.reduce((longest, edge) =>
				Math.hypot(...edge) > Math.hypot(...longest) ? edge : longest,
			);
			return (Math.atan2(Math.abs(dy), Math.abs(dx)) * 180) / Math.PI;
		};
		const low = riser("Stage element 2x1", "front_to_back", 200);
		expect(arms(low)).toHaveLength(2);
		const tallFront = riser("Stage element 2x1", "front_to_back", 1200);
		const tallSide = riser("Stage element 2x1", "left_to_right", 1200);
		expect(arms(tallFront)).toHaveLength(2);
		// From the side the deck is only 1 m deep, so the same rise needs a second X.
		expect(arms(tallSide)).toHaveLength(4);
		for (const [geometry, width, height] of [
			[low, 2000, 200],
			[tallFront, 2000, 1200],
			[tallSide, 1000, 1200],
		] as const) {
			expect(arms(geometry).every((arm) => angle(arm) <= SCISSOR_MAX_DEGREES + 0.5)).toBe(true);
			const points = arms(geometry).flat();
			const xs = points.map(([x]) => x);
			const ys = points.map(([, y]) => y);
			// The arms span nearly the whole deck and stop where the deck and the base frame begin.
			expect(Math.max(...xs) - Math.min(...xs)).toBeGreaterThan(width * 0.92);
			expect(Math.max(...xs)).toBeLessThanOrEqual(width / 2 + 1e-6);
			const deck = Math.min(80, height * 0.3);
			const base = Math.min(50, height * 0.2);
			expect(Math.max(...ys)).toBeCloseTo(height / 2 - deck, 6);
			expect(Math.min(...ys)).toBeCloseTo(-height / 2 + base, 6);
		}
		const top = riser("Stage element 2x1", "top_down", 1200);
		expect(arms(top)).toHaveLength(0);
		expect(top.outlines).toHaveLength(2);

		const stairs = riser("Stage Stairs", "front_to_back", 1200);
		expect(arms(stairs)).toHaveLength(0);
		expect(stairs.outlines).toHaveLength(3);
		expect(arms(riser("Treppe", "left_to_right", 1200, "stage_stairs"))).toHaveLength(0);
	});
});
