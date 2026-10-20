import { expect, it } from "vitest";
import { EntityLayerCache, type CadFrame } from "./lineRenderer";
import { pickEntity, worldGeometry } from "./planGeometry";
import { entityPlanGeometry, type PlanPoint } from "./projection";
import { buildCadPdf, printPageLayout } from "./print";
import { BORDER, TITLE_H } from "./printLayout";
import { printPageHeight, type CadEntity, type CadPrintPage } from "./types";

type Segment = [PlanPoint, PlanPoint];
/** Shared native output: each wall owns its outer contour, with no cap inside the corner. */
function wall(
	id: string,
	plan: Segment[],
	position: [number, number, number] = [0, 0, 0],
	yaw = 0,
): CadEntity {
	return {
		id,
		logicalFixtureId: id,
		name: "Wall",
		fixtureNumber: null,
		fixtureDisplayId: "0.1",
		dmxAddress: "—",
		kind: "venue",
		fixtureType: "venue",
		drawingId: "same-carrier",
		layerId: "default",
		selectable: true,
		positionMillimetres: position,
		rotationDegrees: [0, 0, yaw],
		sizeMillimetres: [4000, 200, 3000],
		outputDirection: [0, 0, 0],
		scenery: {
			kind: "box",
			chords: 0,
			pattern: "standard",
			architecture: { kind: "wall", openings: [] },
		},
		venueGeometry: { triangles: [], plan },
	};
}
const horizontal = () =>
	wall("horizontal", [
		[
			[-2000, 100],
			[2100, 100],
		],
		[
			[-2000, -100],
			[1900, -100],
		],
		[
			[-2000, 100],
			[-2000, -100],
		],
	]);
const vertical = () =>
	wall(
		"vertical",
		[
			[
				[-2100, 100],
				[2000, 100],
			],
			[
				[-1900, -100],
				[2000, -100],
			],
			[
				[2000, 100],
				[2000, -100],
			],
		],
		[2000, 2000, 0],
		90,
	);
const cap: Segment = [
	[2000, 100],
	[2000, -100],
];
function detached() {
	return wall("horizontal", [
		[
			[-2000, 100],
			[2000, 100],
		],
		[
			[-2000, -100],
			[2000, -100],
		],
		[
			[-2000, 100],
			[-2000, -100],
		],
		cap,
	]);
}
function frame(entities: CadEntity[]): CadFrame {
	return {
		entities,
		drawings: new Map(),
		selected: new Set(),
		view: "top_down",
		rotationQuarterTurns: 0,
		camera: { pan: [0, 0], zoom: 1 },
		preview: null,
		editEnabled: true,
		guide: null,
		selectionBox: null,
	};
}
function strokes(vertices: Float32Array): Segment[] {
	const lines: Segment[] = [];
	for (let i = 0; i < vertices.length; i += 14)
		lines.push([
			[vertices[i], vertices[i + 1]],
			[vertices[i + 7], vertices[i + 8]],
		]);
	return lines;
}
function tidy(lines: Segment[]): Segment[] {
	return lines.map(
		(line) =>
			line.map((point) =>
				point.map((v) => Math.round(v * 100) / 100),
			) as Segment,
	);
}
function painted(cache: EntityLayerCache, entities: CadEntity[]) {
	return tidy(strokes(cache.layers(frame(entities)).still.depthLines));
}
const expectedCorner: Segment[] = [
	[
		[-2000, -100],
		[2100, -100],
	],
	[
		[-2000, 100],
		[1900, 100],
	],
	[
		[-2000, -100],
		[-2000, 100],
	],
	[
		[2100, -100],
		[2100, 4000],
	],
	[
		[1900, 100],
		[1900, 4000],
	],
	[
		[2100, 4000],
		[1900, 4000],
	],
];
it("paints a continuous ninety-degree native wall contour without either internal end cap", () => {
	expect(painted(new EntityLayerCache(), [horizontal(), vertical()])).toEqual(
		expectedCorner,
	);
	const local = entityPlanGeometry(vertical(), undefined, "top_down");
	expect(
		tidy(
			worldGeometry(vertical(), local, "top_down", 0).lines.map(
				(l) => l.points,
			),
		),
	).toEqual(expectedCorner.slice(3));
});
it("keeps the individual joined wall IDs selectable at their mitered contours", () => {
	const entities = [horizontal(), vertical()];
	expect(
		pickEntity([2050, -100], entities, new Map(), "top_down", 0, {
			pan: [0, 0],
			zoom: 1,
		})?.id,
	).toBe("horizontal");
	expect(
		pickEntity([2100, 3500], entities, new Map(), "top_down", 0, {
			pan: [0, 0],
			zoom: 1,
		})?.id,
	).toBe("vertical");
});
it("rebuilds an unchanged wall's contextual contour when its neighbour moves or is deleted", () => {
	const cache = new EntityLayerCache();
	expect(painted(cache, [horizontal(), vertical()])).toEqual(expectedCorner);
	const moved = {
		...vertical(),
		positionMillimetres: [8000, 2000, 0] as [number, number, number],
	};
	const movedLines = painted(cache, [detached(), moved]);
	expect(movedLines).toContainEqual([
		[2000, -100],
		[2000, 100],
	]);
	expect(movedLines).not.toContainEqual([
		[-2000, -100],
		[2100, -100],
	]);
	const deletedLines = painted(cache, [detached()]);
	expect(deletedLines).toHaveLength(4);
	expect(deletedLines).toContainEqual([
		[2000, -100],
		[2000, 100],
	]);
	expect(cache.builds.still).toBe(3);
});
const sheet: CadPrintPage = {
	id: "page",
	tileId: "tile",
	name: "Venue",
	view: "top_down",
	rotationQuarterTurns: 0,
	centreMillimetres: [0, 2000],
	widthMillimetres: 12000,
	included: true,
	orientation: "landscape",
	showFixtureIds: false,
	showDmxAddresses: false,
	showMountingHardware: true,
};
function pdfStrokes(
	entities: CadEntity[],
	page: CadPrintPage = sheet,
): Segment[] {
	const pdf = new TextDecoder().decode(
		buildCadPdf(
			{
				showId: "joined",
				sceneRevision: 1,
				selectionRevision: 0,
				entities,
				drawings: [],
				selectedIds: [],
				attachments: [],
			},
			[page],
		),
	);
	const blocks = [
		...pdf.matchAll(/0\.38 G\n0\.65 w\n([\s\S]*?)(?=0\.38 G|\nQ)/g),
	];
	const layout = printPageLayout(page),
		pageHeight = printPageHeight(page);
	const scale = Math.min(
		(layout.widthPoints - 2 * BORDER) / page.widthMillimetres,
		(layout.heightPoints - 2 * BORDER - TITLE_H) / pageHeight,
	);
	const originX = (layout.widthPoints - page.widthMillimetres * scale) / 2;
	const bottom = BORDER + TITLE_H;
	const originY =
		bottom + (layout.heightPoints - bottom - BORDER - pageHeight * scale) / 2;
	const world = (x: number, y: number): PlanPoint => [
		(x - originX) / scale +
			page.centreMillimetres[0] -
			page.widthMillimetres / 2,
		(y - originY) / scale + page.centreMillimetres[1] - pageHeight / 2,
	];
	return blocks.flatMap((block) =>
		[
			...block[1].matchAll(
				/^(-?[\d.]+) (-?[\d.]+) m (-?[\d.]+) (-?[\d.]+) l S$/gm,
			),
		].map(
			(match) =>
				[
					world(Number(match[1]), Number(match[2])),
					world(Number(match[3]), Number(match[4])),
				] as Segment,
		),
	);
}
it("exports exactly the same joined corner and door/window symbols as the CAD painter", () => {
	const symbols: Segment[] = [
		[
			[0, 0],
			[500, 0],
		],
		[
			[500, 0],
			[850, -350],
		],
		[
			[850, -350],
			[1000, -100],
		],
		[
			[-1600, 0],
			[-1200, 0],
		],
	];
	const withSymbols = horizontal();
	withSymbols.venueGeometry = {
		triangles: [],
		plan: [...withSymbols.venueGeometry!.plan, ...symbols],
	};
	const entities = [withSymbols, vertical()];
	const ink = painted(new EntityLayerCache(), entities);
	const pdf = pdfStrokes(entities);
	expect(pdf).toHaveLength(ink.length);
	for (let i = 0; i < ink.length; i++)
		for (let p = 0; p < 2; p++)
			for (let axis = 0; axis < 2; axis++)
				expect(pdf[i][p][axis]).toBeCloseTo(ink[i][p][axis], 0);
	expect(ink).toContainEqual([
		[0, 0],
		[500, 0],
	]);
	expect(ink).toContainEqual([
		[-1600, 0],
		[-1200, 0],
	]);
});

it("paints and exports native exterior edges without restoring collinear ownership caps", () => {
	const joined = horizontal();
	joined.venueGeometry = {
		joinedWall: true,
		plan: joined.venueGeometry!.plan,
		triangles: [
			[
				[-2000, 0, -100],
				[2000, 0, -100],
				[2000, 3000, -100],
			],
			[
				[-2000, 0, -100],
				[2000, 3000, -100],
				[-2000, 3000, -100],
			],
		],
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
		elevation: [
			[
				[-500, 1000, 0],
				[500, 1000, 0],
			],
		],
	};
	const cache = new EntityLayerCache();
	const frontFrame = { ...frame([joined]), view: "front_to_back" as const };
	const ink = tidy(strokes(cache.layers(frontFrame).still.depthLines));
	expect(ink).toEqual([
		[
			[-2000, 0],
			[2000, 0],
		],
		[
			[-2000, 3000],
			[2000, 3000],
		],
		[
			[-500, 1000],
			[500, 1000],
		],
	]);
	const pdf = pdfStrokes([joined], {
		...sheet,
		view: "front_to_back",
		centreMillimetres: [0, 1500],
	});
	expect(pdf).toHaveLength(3);
	for (let i = 0; i < ink.length; i++)
		for (let p = 0; p < 2; p++)
			for (let axis = 0; axis < 2; axis++)
				expect(pdf[i][p][axis]).toBeCloseTo(ink[i][p][axis], 0);
	// Exterior already carries physicaldepth; it must not receive the semanticframe's additional face offset.
	expect(cache.layers(frontFrame).still.depthLines[2]).toBeCloseTo(
		-101 / 100000,
	);
});
