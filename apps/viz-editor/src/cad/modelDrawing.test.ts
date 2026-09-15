import { beforeEach, describe, expect, it, vi } from "vitest";
import { modelDrawingGeometry, parseModelDrawing } from "./modelDrawing";
import { buildCadPdf } from "./print";
import { entityPlanGeometry, type PlanPoint } from "./projection";
import {
	type CadDrawing,
	type CadEntity,
	type CadPrintPage,
	type CadSceneSnapshot,
	directionIndicator,
	projectPoint,
} from "./types";
import { restorePrintPages } from "./useCadPrintPages";

/** A 100 × 80 square with a 40 × 40 hole, one line in `base` and a closed one in `head`. */
const SQUARE = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="-50 -40 100 80">
  <title>square — top</title>
  <g id="silhouette" fill="#1b1f24"><path fill-rule="evenodd" d="M-50 -40 L50 -40 L50 40 L-50 40 Z M-20 -20 L20 -20 L20 20 L-20 20 Z"/></g>
  <g id="lines" fill="none">
    <g id="base"><path d="M-50 -40 L50 -40"/></g>
    <g id="yoke"/>
    <g id="head"><path d="M-20 -20 L20 -20 L20 20 Z"/></g>
  </g>
  <g id="origin"><path d="M-5 0 L5 0"/></g>
</svg>`;

/** The same body with a clamp above it, drawn in a separate file without the clamp. */
const CLAMPED = SQUARE.replace(
	'<g id="yoke"/>',
	'<g id="yoke"><path d="M0 -40 L0 -90"/></g>',
);

const area = (points: readonly PlanPoint[]) =>
	Math.abs(
		(points[1][0] - points[0][0]) * (points[2][1] - points[0][1]) -
			(points[2][0] - points[0][0]) * (points[1][1] - points[0][1]),
	) / 2;

const entity: CadEntity = {
	id: "lamp",
	logicalFixtureId: "lamp",
	name: "Blinder",
	fixtureNumber: 1,
	fixtureDisplayId: "1",
	dmxAddress: "1.1",
	kind: "fixture",
	fixtureType: "blinder",
	drawingId: "blinder:1",
	layerId: "default",
	selectable: true,
	positionMillimetres: [0, 0, 4000],
	rotationDegrees: [0, 0, 0],
	sizeMillimetres: [400, 400, 400],
	outputDirection: [0, 1, 0],
};

const modelDrawing: CadDrawing["modelDrawing"] = {
	model: "square",
	scale: 2,
	views: (["top", "front", "side"] as const).map((view) => ({
		view,
		svg: CLAMPED,
		noClampSvg: SQUARE,
	})),
};

describe("shipped model drawings", () => {
	it("fills the silhouette with triangles that leave its hole open", () => {
		const parsed = parseModelDrawing(SQUARE);
		const covered = parsed.triangles.reduce((sum, t) => sum + area(t), 0);
		expect(covered).toBeCloseTo(100 * 80 - 40 * 40, 6);
		const centroids = parsed.triangles.map(
			(t) =>
				[
					(t[0][0] + t[1][0] + t[2][0]) / 3,
					(t[0][1] + t[1][1] + t[2][1]) / 3,
				] as PlanPoint,
		);
		expect(
			centroids.some(([x, y]) => Math.abs(x) < 20 && Math.abs(y) < 20),
		).toBe(false);
	});

	it("reads the part polylines as lines and ignores the origin cross", () => {
		const parsed = parseModelDrawing(SQUARE);
		// One open segment in base, a closed triangle of three in head.
		expect(parsed.lines).toHaveLength(4);
		// Page y runs down; the plan's runs up.
		expect(parsed.lines[0]).toEqual([
			[-50, 40],
			[50, 40],
		]);
	});

	it("fills a concave ring and a solid inside a hole", () => {
		const parsed = parseModelDrawing(
			`<svg><g id="silhouette"><path d="M0 0 L100 0 L100 100 L50 30 L0 100 Z M200 0 L300 0 L300 100 L200 100 Z M220 20 L280 20 L280 80 L220 80 Z M240 40 L260 40 L260 60 L240 60 Z"/></g></svg>`,
		);
		const covered = parsed.triangles.reduce((sum, t) => sum + area(t), 0);
		const concave = 100 * 100 - (100 * 70) / 2;
		expect(covered).toBeCloseTo(concave + 100 * 100 - 60 * 60 + 20 * 20, 6);
	});

	it("follows a hand-drawn cubic curve along its bend", () => {
		const parsed = parseModelDrawing(
			'<svg><g id="lines"><g id="base"><path d="M0 0 C0 10 10 10 10 0"/></g></g></svg>',
		);
		expect(parsed.lines).toHaveLength(12);
		expect(parsed.lines[0][0]).toEqual([0, -0]);
		expect(parsed.lines[11][1][0]).toBeCloseTo(10, 6);
		expect(parsed.lines[11][1][1]).toBeCloseTo(0, 6);
		// A cubic through (0, 10) and (10, 10) peaks at 7.5 on the page, −7.5 in the plan.
		const lowest = Math.min(...parsed.lines.flat().map(([, y]) => y));
		expect(lowest).toBeCloseTo(-7.5, 1);
	});

	it("follows an arc at its radius", () => {
		const parsed = parseModelDrawing(
			'<svg><g id="lines"><path d="M-10 0 A10 10 0 0 1 10 0"/></g></svg>',
		);
		expect(parsed.lines.length).toBeGreaterThan(4);
		for (const [x, y] of parsed.lines.flat())
			expect(Math.hypot(x, y)).toBeCloseTo(10, 3);
		expect(parsed.lines.at(-1)?.[1]).toEqual([10, -0]);
	});

	it("applies transforms on groups and shapes to rects and lines", () => {
		const parsed = parseModelDrawing(
			`<svg><g id="silhouette" transform="translate(100 0)"><rect x="0" y="0" width="10" height="5" transform="scale(2)"/></g>
			<g id="lines"><g transform="rotate(90)"><line x1="0" y1="0" x2="10" y2="0"/></g></g></svg>`,
		);
		const covered = parsed.triangles.reduce((sum, t) => sum + area(t), 0);
		expect(covered).toBeCloseTo(20 * 10, 6);
		const xs = parsed.triangles.flat().map(([x]) => x);
		expect(Math.min(...xs)).toBeCloseTo(100, 6);
		expect(Math.max(...xs)).toBeCloseTo(120, 6);
		// Turned a quarter turn, the line runs down the page, which is down the plan's y.
		expect(parsed.lines).toHaveLength(1);
		expect(parsed.lines[0][1][0]).toBeCloseTo(0, 6);
		expect(parsed.lines[0][1][1]).toBeCloseTo(-10, 6);
		const moved = parseModelDrawing(
			'<svg><g id="lines" transform="matrix(1 0 0 1 5 0) rotate(90 10 0)"><polyline points="0 0 10 0"/></g></svg>',
		);
		expect(moved.lines[0][0][0]).toBeCloseTo(15, 6);
		expect(moved.lines[0][0][1]).toBeCloseTo(10, 6);
	});

	it("reads a circle as a closed outline at its radius", () => {
		const parsed = parseModelDrawing(
			`<svg><g id="lines"><circle cx="5" cy="5" r="2"/></g><g id="silhouette"><circle cx='0' cy='0' r='10'/></g></svg>`,
		);
		expect(parsed.lines).toHaveLength(36);
		for (const [x, y] of parsed.lines.flat())
			expect(Math.hypot(x - 5, y + 5)).toBeCloseTo(2, 6);
		const covered = parsed.triangles.reduce((sum, t) => sum + area(t), 0);
		expect(covered).toBeGreaterThan(Math.PI * 100 * 0.98);
	});

	it("draws a model drawing ahead of projections and typed symbols, behind live meshes", () => {
		const projection = {
			view: "front" as const,
			svg: '<svg><path d="M0 0 L10 0 L10 10 Z" fill="#808080"/></svg>',
			viewBoxMillimetres: [0, 0, 10, 10] as [number, number, number, number],
			originMillimetres: [0, 0] as [number, number],
		};
		const withDrawing: CadDrawing = {
			id: "blinder:1",
			projections: [projection],
			modelDrawing,
		};
		expect(
			entityPlanGeometry(entity, withDrawing, "front_to_back").source,
		).toBe("model_drawing");
		expect(
			entityPlanGeometry(
				entity,
				{ ...withDrawing, modelDrawing: undefined },
				"front_to_back",
			).source,
		).toBe("model");
		expect(entityPlanGeometry(entity, undefined, "front_to_back").source).toBe(
			"typed",
		);
		const live: CadDrawing = {
			...withDrawing,
			liveMeshes: [
				{
					pose: "elevation",
					triangles: [
						{
							pointsMillimetres: [
								[0, 0, 0],
								[100, 0, 0],
								[0, 100, 0],
							],
							colour: [1, 1, 1],
						},
					],
				},
			],
		};
		expect(entityPlanGeometry(entity, live, "front_to_back").source).toBe(
			"live_model",
		);
		expect(
			entityPlanGeometry(
				{
					...entity,
					scenery: { kind: "truss", chords: 4, pattern: "standard" },
				},
				{ ...withDrawing, projections: [] },
				"front_to_back",
			).source,
		).toBe("typed");
	});

	it("scales to the fixture, mirrors the back and right views and keeps the model's +Z downstage in the plan", () => {
		const drawing: CadDrawing = { id: "d", projections: [], modelDrawing };
		const front = modelDrawingGeometry(drawing, "front_to_back");
		const back = modelDrawingGeometry(drawing, "back_to_front");
		expect(front?.lines[0].points).toEqual([
			[-100, 80],
			[100, 80],
		]);
		expect(back?.lines[0].points).toEqual([
			[100, 80],
			[-100, 80],
		]);
		const top = entityPlanGeometry(entity, drawing, "top_down");
		// Top view: page y is model z, which runs downstage — plan −y — so page y −40 is upstage.
		expect(top.lines[0].points).toEqual([
			[-100, 80],
			[100, 80],
		]);
	});

	it("uses the no-clamp drawing only when mounting hardware is off", () => {
		const drawing: CadDrawing = { id: "d", projections: [], modelDrawing };
		const withHardware = entityPlanGeometry(entity, drawing, "front_to_back");
		const without = entityPlanGeometry(entity, drawing, "front_to_back", {
			mountingHardware: false,
		});
		expect(withHardware.lines).toHaveLength(5);
		expect(without.lines).toHaveLength(4);
		const noVariant: CadDrawing = {
			id: "d",
			projections: [],
			modelDrawing: {
				...modelDrawing,
				views: modelDrawing.views.map(({ view, svg }) => ({ view, svg })),
			},
		};
		expect(
			entityPlanGeometry(entity, noVariant, "front_to_back", {
				mountingHardware: false,
			}).lines,
		).toHaveLength(5);
	});
});

describe("mounting hardware on a print page", () => {
	const scene: CadSceneSnapshot = {
		showId: "show",
		sceneRevision: 1,
		selectionRevision: 1,
		selectedIds: [],
		attachments: [],
		drawings: [{ id: "blinder:1", projections: [], modelDrawing }],
		entities: [entity],
	};
	const page = (showMountingHardware: boolean): CadPrintPage => ({
		kind: "plan",
		id: "page",
		tileId: "tile",
		name: "Page",
		view: "front_to_back",
		rotationQuarterTurns: 0,
		centreMillimetres: [0, 0],
		widthMillimetres: 3000,
		included: true,
		orientation: "landscape",
		showFixtureIds: false,
		showDmxAddresses: false,
		showMountingHardware,
	});
	const strokes = (bytes: Uint8Array) =>
		(new TextDecoder().decode(bytes).match(/ S$/gm) ?? []).length;

	it("prints the no-clamp drawing when the page switches hardware off", () => {
		const on = strokes(buildCadPdf(scene, [page(true)]));
		const off = strokes(buildCadPdf(scene, [page(false)]));
		expect(on - off).toBe(1);
	});

	it("reads a page saved before the switch as printing hardware", () => {
		const store = new Map<string, string>();
		vi.stubGlobal("localStorage", {
			getItem: (key: string) => store.get(key) ?? null,
			setItem: (key: string, value: string) => store.set(key, value),
		});
		const { showMountingHardware: _, ...legacy } = page(true);
		store.set(
			"tosklight:viz-editor:cad-print-pages:v2",
			JSON.stringify([legacy, { ...legacy, id: "off", showMountingHardware: false }]),
		);
		const [restored, off] = restorePrintPages();
		expect(restored.showMountingHardware).toBe(true);
		expect(off.showMountingHardware).toBe(false);
		vi.unstubAllGlobals();
	});
});

/**
 * A side view of a face-forward lamp drawn leaning 20° toward the viewer, the pose bracket −70°
 * gives a face-down lamp: a 240 × 80 body below a hinge 100 mm under the clamp, a line across it
 * and one at its far end, and a hanging bar in front.
 */
const HINGED = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="-100 -50 400 400" data-model="blinder" data-view="side" data-pose="tilted" data-hinge="0 100" data-bracket="-70">
  <g id="silhouette" fill="#1b1f24"><path id="silhouette-body" fill-rule="evenodd" d="M-40 100 L200 100 L200 180 L-40 180 Z"/><path id="silhouette-hardware" fill-rule="evenodd" d="M-10 0 L10 0 L10 120 L-10 120 Z"/></g>
  <g id="lines" fill="none">
    <g id="base"><path d="M-40 110 L200 110"/><path d="M150 100 L150 180"/></g>
    <g id="hardware"><path d="M-10 0 L10 0"/></g>
  </g>
</svg>`;

/** The same drawing hand-edited back to one piece: no hardware group, no hardware silhouette. */
const PLAIN = HINGED.replace(/<path id="silhouette-hardware"[^>]*\/>/, "").replace(
	/<g id="hardware">.*?<\/g>/,
	"",
);

describe("side views turned by the bracket angle", () => {
	const drawingOf = (svg: string): CadDrawing => ({
		id: "d",
		projections: [],
		modelDrawing: {
			model: "blinder",
			scale: 1,
			views: [
				{ view: "side", svg },
				{ view: "front", svg },
			],
		},
	});
	// Hidden-line cuts land within the painter's boundary tolerance of the hardware's edge.
	const has = (
		geometry: { lines: { points: readonly PlanPoint[] }[] } | null,
		[x, y]: PlanPoint,
	) =>
		(geometry?.lines ?? []).some((line) =>
			line.points.some(
				(point) => Math.abs(point[0] - x) < 1e-3 && Math.abs(point[1] - y) < 1e-3,
			),
		);
	/** Where the far body corner at (150, −100) lands after turning `degrees` nose-down. */
	const corner = (degrees: number): PlanPoint => {
		const radians = (degrees * Math.PI) / 180;
		return [150 * Math.cos(radians), -100 - 150 * Math.sin(radians)];
	};
	const side = (svg: string, bracketAngle: number, view = "left_to_right" as const) =>
		entityPlanGeometry({ ...entity, bracketAngle }, drawingOf(svg), view);

	it("reads the hinge, the drawn bracket pose and the hardware apart from the body", () => {
		const parsed = parseModelDrawing(HINGED);
		expect(parsed.hinge).toEqual([0, -100]);
		expect(parsed.bracket).toBe(-70);
		expect(parsed.body.lines).toHaveLength(2);
		expect(parsed.hardware.lines).toHaveLength(1);
		expect(parsed.hardware.triangles.length).toBeGreaterThan(0);
		expect(parsed.triangles).toHaveLength(
			parsed.body.triangles.length + parsed.hardware.triangles.length,
		);
	});

	it("turns the body from its drawn pose to exactly the configured bracket angle, hardware unmoved", () => {
		for (const [bracket, turn] of [
			[-70, 0],
			[0, 70],
			[45, 115],
			[90, 160],
		]) {
			const geometry = side(HINGED, bracket);
			expect(has(geometry, corner(turn)), `bracket ${bracket}`).toBe(true);
			expect(has(geometry, [-10, 0]) && has(geometry, [10, 0])).toBe(true);
			expect(geometry.triangles.length).toBe(
				parseModelDrawing(HINGED).triangles.length,
			);
		}
		// Seen from the other side the same turn is mirrored.
		const [x, y] = corner(115);
		expect(has(side(HINGED, 45, "right_to_left" as never), [-x, y])).toBe(true);
	});

	it("hides the body's lines behind the hanging hardware", () => {
		const geometry = side(HINGED, -70);
		expect(has(geometry, [-10, -110]) && has(geometry, [10, -110])).toBe(true);
		const crossing = geometry.lines.some(
			({ points: [a, b] }) =>
				Math.abs(a[1] + 110) < 1e-6 &&
				Math.abs(b[1] + 110) < 1e-6 &&
				Math.min(a[0], b[0]) < 0 &&
				Math.max(a[0], b[0]) > 0,
		);
		expect(crossing).toBe(false);
	});

	it("turns a hanging drawing by the bracket angle itself", () => {
		const hanging = HINGED.replace(' data-bracket="-70"', "");
		expect(has(side(hanging, 0), corner(0))).toBe(true);
		expect(has(side(hanging, 45), corner(45))).toBe(true);
	});

	it("keeps front views and drawings without separate hardware as drawn", () => {
		expect(
			has(modelDrawingGeometry(drawingOf(HINGED), "front_to_back", true, 45), corner(0)),
		).toBe(true);
		const plain = side(PLAIN, 45);
		expect(parseModelDrawing(PLAIN).hardware.lines).toHaveLength(0);
		expect(has(plain, corner(0))).toBe(true);
		expect(plain.lines).toHaveLength(2);
	});

	it("starts the direction indicator at the lamp's emitter", () => {
		const lamp = {
			...entity,
			emitterOffsetMillimetres: [0, 100, -300] as [number, number, number],
			outputDirection: [0, 0, -1] as [number, number, number],
		};
		expect(directionIndicator(lamp, [1000, 4000], "front_to_back")).toEqual([
			[1000, 3700],
			[1000, 3280],
		]);
		// From house left, upstage (+y) is on the left.
		expect(directionIndicator(lamp, [1000, 4000], "left_to_right")[0]).toEqual([
			900, 3700,
		]);
		// Without a known emitter it starts at the position, as before.
		expect(directionIndicator(entity, [1000, 4000], "front_to_back")[0]).toEqual([
			1000, 4000,
		]);
	});
});

/**
 * The Architect and the Visualizer share one stage convention. The Visualizer's `to_world` puts a
 * desk point (x, y upstage, z up) at renderer (x, z, −y), so a model's +Z — where a lamp faces once
 * turned to the audience — is downstage. Its plan looks down with upstage at the top; its left-to-
 * right camera stands at house left (−x) looking +x with up +Y, so downstage (+Z) is on the right;
 * right to left puts it on the left; front to back shows +x on the right. Every CAD geometry source
 * has to put a lamp's front on those same sides.
 */
describe("upstage and downstage as the Visualizer shows them", () => {
	const lens = (geometry: { triangles: { points: readonly PlanPoint[] }[]; lines: { points: readonly PlanPoint[] }[] }) =>
		[...geometry.triangles, ...geometry.lines].flatMap((part) => part.points);
	const downstage = "the lamp's front, model +Z, 100–200 mm from its origin";

	it(`draws a live model's front downstage in every view: ${downstage}`, () => {
		const drawing: CadDrawing = {
			id: "live",
			projections: [],
			liveMeshes: (["top", "elevation"] as const).map((pose) => ({
				pose,
				triangles: [
					{
						// Not edge-on in any view, so no projection of it is dropped as degenerate.
						pointsMillimetres: [
							[-50, -50, 100],
							[50, 50, 150],
							[0, 50, 200],
						],
						colour: [1, 1, 1],
					},
				],
			})),
		};
		const at = (view: Parameters<typeof entityPlanGeometry>[2]) =>
			lens(entityPlanGeometry(entity, drawing, view));
		expect(entityPlanGeometry(entity, drawing, "top_down").source).toBe("live_model");
		expect(at("top_down").every(([, y]) => y < -99)).toBe(true);
		expect(at("left_to_right").every(([x]) => x > 99)).toBe(true);
		expect(at("right_to_left").every(([x]) => x < -99)).toBe(true);
		// The desk's own projection agrees: a point 150 mm downstage is where the model put it.
		expect(projectPoint([0, -150, 0], "top_down")[1]).toBeLessThan(0);
		expect(projectPoint([0, -150, 0], "left_to_right")[0]).toBeGreaterThan(0);
		expect(projectPoint([0, -150, 0], "right_to_left")[0]).toBeLessThan(0);
		// Turned a quarter about the vertical, the front swings to +x as the Visualizer's Ry turns +Z.
		const turned = lens(
			entityPlanGeometry({ ...entity, rotationDegrees: [0, 0, 90] }, drawing, "top_down"),
		);
		expect(turned.every(([x]) => x > 99)).toBe(true);
	});

	it("draws model drawings and projection SVGs with their front downstage", () => {
		const front = (view: "top" | "side") =>
			`<svg><g id="lines"><g id="base"><path d="${view === "top" ? "M-10 100 L10 200" : "M100 -10 L200 10"}"/></g></g></svg>`;
		const drawing: CadDrawing = {
			id: "drawn",
			projections: [],
			modelDrawing: {
				model: "lamp",
				scale: 1,
				views: [
					{ view: "top", svg: front("top") },
					{ view: "side", svg: front("side") },
				],
			},
		};
		expect(lens(entityPlanGeometry(entity, drawing, "top_down")).every(([, y]) => y < -99)).toBe(true);
		expect(lens(entityPlanGeometry(entity, drawing, "left_to_right")).every(([x]) => x > 99)).toBe(true);
		expect(lens(entityPlanGeometry(entity, drawing, "right_to_left")).every(([x]) => x < -99)).toBe(true);
		// A package's projection SVGs are drawn the same way: from above, page y is +Z; from the
		// left, page x is +Z; from the right, page x is −Z.
		const polygon = (d: string) => `<svg><path d="${d}" fill="#808080"/></svg>`;
		const projected: CadDrawing = {
			id: "projected",
			projections: [
				{ view: "top", svg: polygon("M-10 100 L10 100 L0 200 Z"), viewBoxMillimetres: [0, 0, 1, 1], originMillimetres: [0, 0] },
				{ view: "left", svg: polygon("M100 -10 L100 10 L200 0 Z"), viewBoxMillimetres: [0, 0, 1, 1], originMillimetres: [0, 0] },
				{ view: "right", svg: polygon("M-100 -10 L-100 10 L-200 0 Z"), viewBoxMillimetres: [0, 0, 1, 1], originMillimetres: [0, 0] },
			],
		};
		const typedFixture = { ...entity, fixtureType: "unknown gadget" };
		expect(lens(entityPlanGeometry(typedFixture, projected, "top_down")).every(([, y]) => y < -99)).toBe(true);
		expect(lens(entityPlanGeometry(typedFixture, projected, "left_to_right")).every(([x]) => x > 99)).toBe(true);
		expect(lens(entityPlanGeometry(typedFixture, projected, "right_to_left")).every(([x]) => x < -99)).toBe(true);
	});

	it("draws a typed conventional's lens downstage in the plan", () => {
		const typed = entityPlanGeometry(entity, undefined, "top_down");
		const dark = typed.triangles.filter(({ color }) => color.join(",") === "0.13,0.15,0.18");
		const centre = dark.flatMap(({ points }) => points).reduce((sum, [, y]) => sum + y, 0);
		expect(centre).toBeLessThan(0);
	});

	it("points a downstage lamp's indicator downstage from its lens", () => {
		// What the Rust side sends for a lamp turned to face the audience: plan −y.
		const lamp = {
			...entity,
			emitterOffsetMillimetres: [0, -100, -200] as [number, number, number],
			outputDirection: [0, -1, 0] as [number, number, number],
		};
		const [start, end] = directionIndicator(lamp, [0, 0], "top_down");
		expect(start[1]).toBe(-100);
		expect(end[1]).toBeLessThan(start[1]);
		const [sideStart, sideEnd] = directionIndicator(lamp, [0, 0], "left_to_right");
		expect(sideStart[0]).toBe(100);
		expect(sideEnd[0]).toBeGreaterThan(sideStart[0]);
	});
});

beforeEach(() => vi.unstubAllGlobals());
