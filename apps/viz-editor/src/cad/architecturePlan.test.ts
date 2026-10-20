import { describe, expect, it } from "vitest";
import { pickEntity, worldGeometry } from "./planGeometry";
import { buildCadPdf, printPageLayout } from "./print";
import { BORDER, TITLE_H } from "./printLayout";
import { entityPlanGeometry } from "./projection";
import type {
	CadEntity,
	CadPrintPage,
	CadSceneSnapshot,
	CadViewDirection,
} from "./types";
import { printPageHeight } from "./types";

/** Native venue geometry, with a real door gap and three segments of its swing arc. */
const wall: CadEntity = {
	id: "wall",
	logicalFixtureId: "wall",
	name: "Venue wall",
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
	sizeMillimetres: [6000, 3000, 200],
	outputDirection: [0, 0, 0],
	scenery: {
		kind: "box",
		chords: 0,
		pattern: "standard",
		architecture: { kind: "wall", openings: [] },
	},
	venueGeometry: {
		triangles: [
			[
				[-3000, 0, -100],
				[-1000, 0, -100],
				[-3000, 3000, -100],
			],
			[
				[-1000, 0, -100],
				[-1000, 3000, -100],
				[-3000, 3000, -100],
			],
		],
		plan: [
			[
				[-3000, -100],
				[-1000, -100],
			],
			[
				[0, -100],
				[3000, -100],
			],
			[
				[-1000, -100],
				[-1000, -1100],
			],
			[
				[-1000, -1100],
				[-293, -807],
			],
			[
				[-293, -807],
				[0, -100],
			],
		],
	},
};
const page = (view: CadViewDirection): CadPrintPage => ({
	id: view,
	tileId: "tile",
	name: view,
	view,
	rotationQuarterTurns: 0,
	centreMillimetres: [1000, 2000],
	widthMillimetres: 12000,
	included: true,
	orientation: "landscape",
	showFixtureIds: false,
	showDmxAddresses: false,
	showMountingHardware: true,
});
function scene(entity: CadEntity): CadSceneSnapshot {
	return {
		showId: "venue",
		sceneRevision: 1,
		selectionRevision: 1,
		selectedIds: [],
		attachments: [],
		drawings: [],
		entities: [entity],
	};
}

describe("venue architecture plans", () => {
	it("uses native wall segments and door swing arcs without filling the opening with a box", () => {
		const geometry = entityPlanGeometry(wall, undefined, "top_down");
		expect(geometry.source).toBe("live_model");
		expect(geometry.triangles).toEqual([]);
		expect(geometry.outlines).toEqual([]);
		expect(geometry.lines.map((line) => line.points)).toEqual([
			[
				[-3000, 100],
				[-1000, 100],
			],
			[
				[0, 100],
				[3000, 100],
			],
			[
				[-1000, 100],
				[-1000, 1100],
			],
			[
				[-1000, 1100],
				[-293, 807],
			],
			[
				[-293, 807],
				[0, 100],
			],
		]);
	});
	it("projects native wall faces at the actual floor elevation", () => {
		const projected = entityPlanGeometry(wall, undefined, "front_to_back");
		expect(projected.source).toBe("live_model");
		expect(projected.triangles).toHaveLength(2);
		const world = worldGeometry(wall, projected, "front_to_back", 0);
		const points = world.triangles.flatMap((triangle) => triangle.points);
		expect(Math.min(...points.map((point) => point[1]))).toBe(500);
		expect(Math.max(...points.map((point) => point[1]))).toBe(3500);
		expect(Math.min(...points.map((point) => point[0]))).toBe(-2000);
	});
	it("applies a wall's yaw once, then adds its world position", () => {
		const rotated = {
			...wall,
			rotationDegrees: [0, 0, 90] as [number, number, number],
		};
		const world = worldGeometry(
			rotated,
			entityPlanGeometry(rotated, undefined, "top_down"),
			"top_down",
			0,
		);
		expect(world.lines[0].points[0][0]).toBeCloseTo(900);
		expect(world.lines[0].points[0][1]).toBeCloseTo(-1000);
		expect(world.lines[0].points[1][0]).toBeCloseTo(900);
		expect(world.lines[0].points[1][1]).toBeCloseTo(1000);
	});
	for (const view of ["top_down", "front_to_back"] as const)
		it(`exports ${view} with the same physical line lengths and directions as the CAD drawing`, () => {
			const sheet = page(view);
			const pdf = new TextDecoder().decode(buildCadPdf(scene(wall), [sheet]));
			const entityContent = pdf.split("0.38 G\n0.65 w\n")[1]?.split("\nQ")[0];
			expect(entityContent).toBeDefined();
			if (!entityContent) throw new Error("Missing venue drawing in PDF");
			const paths = [
				...entityContent.matchAll(
					/^(-?[\d.]+) (-?[\d.]+) m (-?[\d.]+) (-?[\d.]+) l S$/gm,
				),
			].map((match) => match.slice(1).map(Number));
			const projected = worldGeometry(
				wall,
				entityPlanGeometry(wall, undefined, view),
				view,
				0,
			);
			expect(paths).toHaveLength(projected.lines.length);
			expect(paths.length).toBeGreaterThan(0);
			const layout = printPageLayout(sheet);
			const scale = Math.min(
				(layout.widthPoints - 2 * BORDER) / sheet.widthMillimetres,
				(layout.heightPoints - 2 * BORDER - TITLE_H) / printPageHeight(sheet),
			);
			for (const [index, line] of projected.lines.entries()) {
				const [x1, y1, x2, y2] = paths[index];
				expect((x2 - x1) / scale).toBeCloseTo(
					line.points[1][0] - line.points[0][0],
					0,
				);
				expect((y2 - y1) / scale).toBeCloseTo(
					line.points[1][1] - line.points[0][1],
					0,
				);
			}
		});
});

describe("venue picking follows its drawn geometry", () => {
	const camera = { pan: [0, 0] as [number, number], zoom: 1 };
	it("selects the far end of a long wall by its visible line rather than only its centre", () => {
		const picked = pickEntity(
			[-1900, 2105],
			[wall],
			new Map(),
			"top_down",
			0,
			camera,
		);
		expect(picked?.id).toBe(wall.id);
	});
	it("leaves a doorway at the entity centre open to selection behind it", () => {
		const openWall = {
			...wall,
			venueGeometry: {
				triangles: [],
				plan: [
					[
						[-3000, -100],
						[-500, -100],
					],
					[
						[500, -100],
						[3000, -100],
					],
				],
			},
		} as CadEntity;
		expect(
			pickEntity([1000, 2000], [openWall], new Map(), "top_down", 0, camera),
		).toBeNull();
	});
	it("selects a balcony inside its custom footprint away from its placement point", () => {
		const balcony = {
			...wall,
			scenery: {
				kind: "box",
				chords: 0,
				pattern: "standard",
				architecture: {
					kind: "slab",
					purpose: "balcony",
					outline_millimetres: [
						{ x: -1000, y: -2000 },
						{ x: 5000, y: -2000 },
						{ x: 5000, y: 2000 },
						{ x: -1000, y: 2000 },
					],
					elevation_millimetres: 4200,
					thickness_millimetres: 300,
				},
			},
			venueGeometry: {
				triangles: [],
				plan: [
					[
						[-1000, -2000],
						[5000, -2000],
					],
					[
						[5000, -2000],
						[5000, 2000],
					],
					[
						[5000, 2000],
						[-1000, 2000],
					],
					[
						[-1000, 2000],
						[-1000, -2000],
					],
				],
			},
		} as CadEntity;
		expect(
			pickEntity([5500, 3500], [balcony], new Map(), "top_down", 0, camera)?.id,
		).toBe(balcony.id);
		expect(
			pickEntity(
				[5500, 3500],
				[{ ...balcony, selectable: false }],
				new Map(),
				"top_down",
				0,
				camera,
			),
		).toBeNull();
	});
});
