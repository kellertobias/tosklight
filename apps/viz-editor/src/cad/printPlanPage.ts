/**
 * One plan page of the printed PDF: the sheet's grid, the slice of the rig it shows, and the
 * border and title block around it.
 *
 * The page's own `point()` maps plan millimetres onto the sheet, so everything drawn inside the
 * clip — fixtures today, placed drawings next — lands at the page's scale.
 */
import { visibleEntities } from "./cutPlanes";
import {
	BORDER,
	type CadPrintDocumentInfo,
	printGridMillimetres,
	printPageLayout,
	printScaleDenominator,
	TITLE_H,
	TITLE_W,
} from "./printLayout";
import {
	distance,
	n,
	path,
	saved,
	text,
	titleMark,
} from "./printPdfOps";
import { parseCompanyLogo } from "../document/companyLogo";
import { entityPlanGeometry, type PlanPoint } from "./projection";
import {
	annotationLabels,
	annotationRuns,
	annotationsForView,
} from "./annotationGeometry";
import type { CadAnnotation } from "./annotations";
import { planTransform } from "./planGeometry";
import { placedPolylines, underlaysForPage } from "./underlayGeometry";
import type { CadUnderlay } from "./underlays";
import {
	type CadDrawing,
	type CadEntity,
	type CadPrintPage,
	type CadSceneSnapshot,
	directionIndicator,
	printPageHeight,
	projectPoint,
} from "./types";

export interface PrintPageStream {
	content: string;
	width: number;
	height: number;
}

/** Where a page's plan window lands on the sheet, in PDF points. */
interface PageFrame {
	width: number;
	height: number;
	pageHeight: number;
	drawingBottom: number;
	scale: number;
	originX: number;
	originY: number;
	grid: number;
	point(value: PlanPoint): PlanPoint;
}

function pageFrame(page: CadPrintPage): PageFrame {
	const layout = printPageLayout(page);
	const width = layout.widthPoints;
	const height = layout.heightPoints;
	const pageHeight = printPageHeight(page);
	const drawingBottom = BORDER + TITLE_H;
	const scale = Math.min(
		(width - BORDER * 2) / page.widthMillimetres,
		(height - drawingBottom - BORDER) / pageHeight,
	);
	const originX = (width - page.widthMillimetres * scale) / 2;
	const originY =
		drawingBottom + (height - drawingBottom - BORDER - pageHeight * scale) / 2;
	return {
		width,
		height,
		pageHeight,
		drawingBottom,
		scale,
		originX,
		originY,
		grid: printGridMillimetres(page),
		point: (value) => [
			originX +
				(value[0] - page.centreMillimetres[0] + page.widthMillimetres / 2) *
					scale,
			originY + (value[1] - page.centreMillimetres[1] + pageHeight / 2) * scale,
		],
	};
}

/** The white sheet and the measured grid over it. */
function gridCommands(page: CadPrintPage, frame: PageFrame): string[] {
	const { grid, point } = frame;
	const commands = [
		"q",
		"1 1 1 rg",
		`0 0 ${n(frame.width)} ${n(frame.height)} re f`,
		"0.9 G",
		"0.3 w",
	];
	const left = page.centreMillimetres[0] - page.widthMillimetres / 2;
	const bottom = page.centreMillimetres[1] - frame.pageHeight / 2;
	for (
		let x = Math.ceil(left / grid) * grid;
		x <= left + page.widthMillimetres;
		x += grid
	)
		commands.push(
			path(
				[point([x, bottom]), point([x, bottom + frame.pageHeight])],
				false,
				false,
			),
		);
	for (
		let y = Math.ceil(bottom / grid) * grid;
		y <= bottom + frame.pageHeight;
		y += grid
	)
		commands.push(
			path(
				[point([left, y]), point([left + page.widthMillimetres, y])],
				false,
				false,
			),
		);
	return commands;
}

/** The venue drawings this page prints, in a lighter line than the rig over them. */
function underlayCommands(
	underlays: readonly CadUnderlay[],
	page: CadPrintPage,
	point: PageFrame["point"],
): string[] {
	const commands: string[] = [];
	for (const underlay of underlaysForPage(underlays, page)) {
		commands.push("0.55 G", "0.4 w");
		for (const run of placedPolylines(underlay, page.rotationQuarterTurns))
			commands.push(path(run.map(point), false, false));
	}
	return commands;
}

/** What the operator drew on this page's view: lines and measurements over the rig, and words. */
function annotationCommands(
	annotations: readonly CadAnnotation[],
	page: CadPrintPage,
	frame: PageFrame,
): string[] {
	const drawn = annotationsForView(annotations, page.view);
	if (!drawn.length) return [];
	const commands = ["0.1 G", "0.1 g", "0.6 w"];
	for (const annotation of drawn)
		for (const run of annotationRuns(annotation, page.rotationQuarterTurns))
			commands.push(path(run.map(frame.point), false, false));
	for (const label of annotationLabels(drawn, page.rotationQuarterTurns)) {
		const [x, y] = frame.point(label.point);
		// Text prints at its own height on the plan; a distance at a size the page can read.
		const size = label.heightMillimetres
			? label.heightMillimetres * frame.scale
			: 7;
		commands.push(
			label.kind === "measure"
				? text(label.text, x - label.text.length * size * 0.25, y + 2, size)
				: text(label.text, x, y, size),
		);
	}
	return commands;
}

/** One fixture: its outline, its linework, its beam direction and the labels the page asked for. */
function entityCommands(
	entity: CadEntity,
	drawing: CadDrawing | undefined,
	page: CadPrintPage,
	point: PageFrame["point"],
): string[] {
	const geometry = entityPlanGeometry(entity, drawing, page.view, {
		mountingHardware: page.showMountingHardware !== false,
	});
	// The same transform the screen draws with, so a printed plan cannot drift from the view.
	const transform = planTransform(
		entity,
		geometry,
		page.view,
		page.rotationQuarterTurns,
	);
	// The drawing's own origin: where the entity stands on the page.
	const centre = transform([0, 0]);
	const commands = [entity.kind === "venue" ? "0.38 G" : "0.12 G", "0.65 w"];
	for (const outline of geometry.outlines)
		commands.push(
			path(
				outline.map((p) => point(transform(p))),
				false,
			),
		);
	for (const modelLine of geometry.lines)
		commands.push(
			path(
				modelLine.points.map((p) => point(transform(p))),
				false,
				false,
			),
		);
	if (entity.kind === "venue") return commands;
	const [start, end] = directionIndicator(
		entity,
		centre,
		page.view,
		page.rotationQuarterTurns,
	);
	commands.push(path([point(start), point(end)], false, false));
	const labels = [
		page.showFixtureIds ? `ID ${entity.fixtureDisplayId}` : "",
		page.showDmxAddresses && entity.dmxAddress !== "—"
			? `DMX ${entity.dmxAddress}`
			: "",
	].filter(Boolean);
	if (!labels.length) return commands;
	commands.push("0 0.71 0.92 rg");
	for (const [index, label] of labels.entries())
		commands.push(
			text(
				label,
				point(centre)[0] + 3,
				point(centre)[1] + 3 - index * 8,
				6,
				true,
			),
		);
	return commands;
}

/** The border, the title block and the scale legend around the drawing. */
function furnitureCommands(
	page: CadPrintPage,
	info: CadPrintDocumentInfo,
	frame: PageFrame,
): string[] {
	const { width: W, height: H, drawingBottom } = frame;
	const tx = W - BORDER - TITLE_W;
	const details = [
		["Project", info.project],
		["Show", info.showName],
		["Lighting designer", info.lightingDesigner],
		["Venue", info.venue],
		["Show date", info.showDate],
		[
			"Contact",
			[info.contactEmail, info.contactPhone].filter(Boolean).join(" / "),
		],
		["Version", info.showVersion],
	].filter((entry) => entry[1]);
	return [
		"Q",
		"0.08 G",
		"0.9 w",
		`${n(BORDER)} ${n(BORDER)} ${n(W - BORDER * 2)} ${n(H - BORDER * 2)} re S`,
		`${n(BORDER)} ${n(drawingBottom)} ${n(W - BORDER * 2)} ${n(H - drawingBottom - BORDER)} re S`,
		"1 1 1 rg",
		`${n(tx)} ${n(BORDER)} ${n(TITLE_W)} ${n(TITLE_H)} re f`,
		"0.08 G",
		`${n(tx)} ${n(BORDER)} ${n(TITLE_W)} ${n(TITLE_H)} re S`,
		`${n(tx + 70)} ${n(BORDER)} 0 ${n(TITLE_H)} re S`,
		`${n(tx + 70)} ${n(BORDER + TITLE_H / 2)} ${n(TITLE_W - 70)} 0 re S`,
		...titleMark(parseCompanyLogo(info.companyLogo), tx + 12, BORDER + 18),
		text("ToskLight Architect", tx + 76, BORDER + TITLE_H - 15, 10, true),
		...details
			.slice(0, 4)
			.map(([label, value], index) =>
				text(
					`${label}  ${value}`,
					tx + 76,
					BORDER + TITLE_H - 29 - index * 10,
					6.5,
					index === 0,
				),
			),
		...details
			.slice(4)
			.map(([label, value], index) =>
				text(
					`${label}  ${value}`,
					tx + 190,
					BORDER + TITLE_H - 29 - index * 10,
					6.5,
				),
			),
		text(`Saved  ${saved(info.lastSavedAt)}`, tx + 190, BORDER + 29, 7),
		text(
			`${info.fixtureCount} fixtures / ${info.universeCount} universes`,
			tx + 190,
			BORDER + 17,
			7,
		),
		text(
			`Scale 1:${printScaleDenominator(page)} / Grid ${distance(frame.grid)}`,
			BORDER + 5,
			BORDER + 8,
			7,
		),
		"Q",
	];
}

/** Compose one plan page into a PDF content stream. */
export function planPageStream(
	scene: CadSceneSnapshot,
	drawings: ReadonlyMap<string, CadDrawing>,
	page: CadPrintPage,
	info: CadPrintDocumentInfo,
	underlays: readonly CadUnderlay[] = [],
	annotations: readonly CadAnnotation[] = [],
): PrintPageStream[] {
	const frame = pageFrame(page);
	const commands = gridCommands(page, frame);
	commands.push(
		"q",
		`${n(frame.originX)} ${n(frame.originY)} ${n(page.widthMillimetres * frame.scale)} ${n(frame.pageHeight * frame.scale)} re W n`,
	);
	commands.push(...underlayCommands(underlays, page, frame.point));
	for (const entity of visibleEntities(
		scene.entities,
		page.view,
		page.cutPlanes,
	))
		commands.push(
			...entityCommands(
				entity,
				drawings.get(entity.drawingId),
				page,
				frame.point,
			),
		);
	commands.push(...annotationCommands(annotations, page, frame));
	commands.push(...furnitureCommands(page, info, frame));
	return [
		{
			content: commands.join("\n"),
			width: frame.width,
			height: frame.height,
		},
	];
}
