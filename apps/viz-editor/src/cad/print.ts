import { entityPlanGeometry, type PlanPoint } from "./projection";
import {
	type CadPrintPage,
	type CadSceneSnapshot,
	printPageHeight,
	printPaperSize,
	projectPoint,
} from "./types";

const PT_MM = 72 / 25.4;
export const PRINT_BORDER_MM = 7;
export const PRINT_TITLE_WIDTH_MM = 116.42;
export const PRINT_TITLE_HEIGHT_MM = 30;
const BORDER = PRINT_BORDER_MM * PT_MM;
const TITLE_H = PRINT_TITLE_HEIGHT_MM * PT_MM;
const TITLE_W = PRINT_TITLE_WIDTH_MM * PT_MM;

export interface CadPrintDocumentInfo {
	showName: string;
	lightingDesigner: string;
	showVersion: string;
	venue: string;
	contactEmail: string;
	contactPhone: string;
	project: string;
	showDate: string;
	lastSavedAt: number;
	fixtureCount: number;
	universeCount: number;
}

export function printScaleDenominator(
	page: Pick<CadPrintPage, "widthMillimetres" | "orientation">,
) {
	const paper = printPaperSize(page);
	return Math.max(
		1,
		Math.round(page.widthMillimetres / (paper.width - PRINT_BORDER_MM * 2)),
	);
}

export function printPageLayout(page: Pick<CadPrintPage, "orientation">) {
	const paper = printPaperSize(page);
	return {
		widthPoints: paper.width * PT_MM,
		heightPoints: paper.height * PT_MM,
		paperWidthMillimetres: paper.width,
		paperHeightMillimetres: paper.height,
		titleWidthMillimetres: PRINT_TITLE_WIDTH_MM,
		titleHeightMillimetres: PRINT_TITLE_HEIGHT_MM,
	};
}

export function rotatePrintPage(page: CadPrintPage): CadPrintPage {
	const oldPaper = printPaperSize(page);
	const orientation =
		page.orientation === "portrait" ? "landscape" : "portrait";
	const nextPaper = printPaperSize({ orientation });
	return {
		...page,
		orientation,
		widthMillimetres:
			page.widthMillimetres * (nextPaper.width / oldPaper.width),
	};
}

export function printGridMillimetres(
	page: Pick<CadPrintPage, "widthMillimetres">,
) {
	const target = page.widthMillimetres / 12;
	const magnitude = 10 ** Math.floor(Math.log10(Math.max(1, target)));
	return (
		[1, 2, 5, 10]
			.map((value) => value * magnitude)
			.find((value) => value >= target) ?? magnitude * 10
	);
}

export function buildCadPdf(
	scene: CadSceneSnapshot,
	pages: readonly CadPrintPage[],
	info: CadPrintDocumentInfo = {
		showName: "",
		lightingDesigner: "",
		showVersion: "",
		venue: "",
		contactEmail: "",
		contactPhone: "",
		project: "",
		showDate: "",
		lastSavedAt: 0,
		fixtureCount: scene.entities.length,
		universeCount: 0,
	},
): Uint8Array {
	const drawings = new Map(
		scene.drawings.map((drawing) => [drawing.id, drawing]),
	);
	return pdfDocument(
		pages.map((page) => {
			const layout = printPageLayout(page);
			const W = layout.widthPoints;
			const H = layout.heightPoints;
			const pageHeight = printPageHeight(page);
			const drawingBottom = BORDER + TITLE_H;
			const scale = Math.min(
				(W - BORDER * 2) / page.widthMillimetres,
				(H - drawingBottom - BORDER) / pageHeight,
			);
			const originX = (W - page.widthMillimetres * scale) / 2;
			const originY =
				drawingBottom + (H - drawingBottom - BORDER - pageHeight * scale) / 2;
			const point = (value: PlanPoint): PlanPoint => [
				originX +
					(value[0] - page.centreMillimetres[0] + page.widthMillimetres / 2) *
						scale,
				originY +
					(value[1] - page.centreMillimetres[1] + pageHeight / 2) * scale,
			];
			const commands = [
				"q",
				"1 1 1 rg",
				`0 0 ${n(W)} ${n(H)} re f`,
				"0.9 G",
				"0.3 w",
			];
			const grid = printGridMillimetres(page);
			const left = page.centreMillimetres[0] - page.widthMillimetres / 2;
			const bottom = page.centreMillimetres[1] - pageHeight / 2;
			for (
				let x = Math.ceil(left / grid) * grid;
				x <= left + page.widthMillimetres;
				x += grid
			)
				commands.push(
					path(
						[point([x, bottom]), point([x, bottom + pageHeight])],
						false,
						false,
					),
				);
			for (
				let y = Math.ceil(bottom / grid) * grid;
				y <= bottom + pageHeight;
				y += grid
			)
				commands.push(
					path(
						[point([left, y]), point([left + page.widthMillimetres, y])],
						false,
						false,
					),
				);

			commands.push(
				"q",
				`${n(originX)} ${n(originY)} ${n(page.widthMillimetres * scale)} ${n(pageHeight * scale)} re W n`,
			);
			for (const entity of scene.entities) {
				const geometry = entityPlanGeometry(
					entity,
					drawings.get(entity.drawingId),
					page.view,
				);
				const transform = (local: PlanPoint): PlanPoint => {
					const centre = projectPoint(
						entity.positionMillimetres,
						page.view,
						page.rotationQuarterTurns,
					);
					const angle =
						page.view === "top_down"
							? (((geometry.source === "live_model"
									? 0
									: -entity.rotationDegrees[2]) +
									page.rotationQuarterTurns * 90) *
									Math.PI) /
								180
							: 0;
					return [
						centre[0] + local[0] * Math.cos(angle) - local[1] * Math.sin(angle),
						centre[1] + local[0] * Math.sin(angle) + local[1] * Math.cos(angle),
					];
				};
				commands.push(entity.kind === "venue" ? "0.38 G" : "0.12 G", "0.65 w");
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
				if (entity.kind !== "venue") {
					const centre = projectPoint(
						entity.positionMillimetres,
						page.view,
						page.rotationQuarterTurns,
					);
					const direction = projectPoint(
						entity.outputDirection.map((v) => v * 420) as [
							number,
							number,
							number,
						],
						page.view,
						page.rotationQuarterTurns,
					);
					commands.push(
						path(
							[
								point(centre),
								point([centre[0] + direction[0], centre[1] + direction[1]]),
							],
							false,
							false,
						),
					);
					const labels = [
						page.showFixtureIds ? `ID ${entity.fixtureDisplayId}` : "",
						page.showDmxAddresses && entity.dmxAddress !== "—"
							? `DMX ${entity.dmxAddress}`
							: "",
					].filter(Boolean);
					if (labels.length) {
						commands.push("0 0.71 0.92 rg");
						labels.forEach((label, index) =>
							commands.push(
								text(
									label,
									point(centre)[0] + 3,
									point(centre)[1] + 3 - index * 8,
									6,
									true,
								),
							),
						);
					}
				}
			}
			commands.push(
				"Q",
				"0.08 G",
				"0.9 w",
				`${n(BORDER)} ${n(BORDER)} ${n(W - BORDER * 2)} ${n(H - BORDER * 2)} re S`,
				`${n(BORDER)} ${n(drawingBottom)} ${n(W - BORDER * 2)} ${n(H - drawingBottom - BORDER)} re S`,
			);
			const tx = W - BORDER - TITLE_W;
			commands.push(
				"1 1 1 rg",
				`${n(tx)} ${n(BORDER)} ${n(TITLE_W)} ${n(TITLE_H)} re f`,
				"0.08 G",
				`${n(tx)} ${n(BORDER)} ${n(TITLE_W)} ${n(TITLE_H)} re S`,
				`${n(tx + 70)} ${n(BORDER)} 0 ${n(TITLE_H)} re S`,
				`${n(tx + 70)} ${n(BORDER + TITLE_H / 2)} ${n(TITLE_W - 70)} 0 re S`,
				...mark(tx + 12, BORDER + 18),
			);
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
			commands.push(
				text("Tasklight Architect", tx + 76, BORDER + TITLE_H - 15, 10, true),
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
					`Scale 1:${printScaleDenominator(page)} / Grid ${distance(grid)}`,
					BORDER + 5,
					BORDER + 8,
					7,
				),
				"Q",
			);
			return { content: commands.join("\n"), width: W, height: H };
		}),
	);
}

function mark(x: number, y: number) {
	return [
		"0.02 0.71 1 rg",
		`${n(x)} ${n(y + 22)} m ${n(x + 16)} ${n(y + 31)} l ${n(x + 16)} ${n(y + 13)} l h f`,
		"0.09 0.12 0.15 rg",
		`${n(x + 16)} ${n(y + 12)} m ${n(x + 49)} ${n(y + 30)} l ${n(x + 49)} ${n(y - 1)} l ${n(x + 16)} ${n(y + 8)} l h f`,
	];
}
function saved(seconds: number) {
	return seconds
		? `${new Date(seconds * 1000).toISOString().slice(0, 16).replace("T", " ")} UTC`
		: "-";
}
function distance(mm: number) {
	return mm >= 1000 ? `${n(mm / 1000)} m` : `${n(mm)} mm`;
}
function text(value: string, x: number, y: number, size: number, bold = false) {
	return `BT /${bold ? "F2" : "F1"} ${size} Tf ${n(x)} ${n(y)} Td (${value.replace(/[^\x20-\xff]/g, "-").replace(/([\\()])/g, "\\$1")}) Tj ET`;
}
function path(points: readonly PlanPoint[], fill: boolean, close = true) {
	if (!points.length) return "";
	const result = [`${n(points[0][0])} ${n(points[0][1])} m`];
	for (const p of points.slice(1)) result.push(`${n(p[0])} ${n(p[1])} l`);
	if (close) result.push("h");
	result.push(fill ? "f" : "S");
	return result.join(" ");
}
function n(value: number) {
	return Number.isFinite(value) ? value.toFixed(2).replace(/\.00$/, "") : "0";
}
function pdfDocument(
	streams: readonly { content: string; width: number; height: number }[],
) {
	const objects: string[] = [];
	const ids = streams.map((_, index) => 3 + index * 2);
	objects.push(
		"<< /Type /Catalog /Pages 2 0 R >>",
		`<< /Type /Pages /Count ${streams.length} /Kids [${ids.map((id) => `${id} 0 R`).join(" ")}] >>`,
	);
	for (let i = 0; i < streams.length; i++) {
		const contentId = ids[i] + 1;
		objects.push(
			`<< /Type /Page /Parent 2 0 R /MediaBox [0 0 ${n(streams[i].width)} ${n(streams[i].height)}] /Resources << /Font << /F1 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> /F2 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold >> >> >> /Contents ${contentId} 0 R >>`,
			`<< /Length ${new TextEncoder().encode(streams[i].content).length} >>\nstream\n${streams[i].content}\nendstream`,
		);
	}
	let output = "%PDF-1.4\n%Tasklight Architect\n";
	const offsets = [0];
	for (let i = 0; i < objects.length; i++) {
		offsets.push(new TextEncoder().encode(output).length);
		output += `${i + 1} 0 obj\n${objects[i]}\nendobj\n`;
	}
	const xref = new TextEncoder().encode(output).length;
	output += `xref\n0 ${objects.length + 1}\n0000000000 65535 f \n`;
	for (const offset of offsets.slice(1))
		output += `${String(offset).padStart(10, "0")} 00000 n \n`;
	output += `trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
	return new TextEncoder().encode(output);
}
