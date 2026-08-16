import { entityPlanGeometry, type PlanPoint } from "./projection";
import {
	type CadPrintPage,
	type CadSceneSnapshot,
	printPageHeight,
	projectPoint,
} from "./types";

const W = 841.89;
const H = 595.28;
const PT_MM = 72 / 25.4;
const BORDER = 7 * PT_MM;
const TITLE_H = 30 * PT_MM;

export interface CadPrintDocumentInfo {
	showFileName: string;
	lightingDesigner: string;
	showVersion: string;
	lastSavedAt: number;
	fixtureCount: number;
	universeCount: number;
}

export function printScaleDenominator(
	page: Pick<CadPrintPage, "widthMillimetres">,
) {
	return Math.max(
		1,
		Math.round(page.widthMillimetres / ((W - BORDER * 2) / PT_MM)),
	);
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
		showFileName: "Untitled.show",
		lightingDesigner: "",
		showVersion: "",
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
							? ((-entity.rotationDegrees[2] + page.rotationQuarterTurns * 90) *
									Math.PI) /
								180
							: 0;
					return [
						centre[0] + local[0] * Math.cos(angle) - local[1] * Math.sin(angle),
						centre[1] + local[0] * Math.sin(angle) + local[1] * Math.cos(angle),
					];
				};
				commands.push(entity.kind === "venue" ? "0.82 g" : "0.68 g");
				for (const triangle of geometry.triangles)
					commands.push(
						path(
							triangle.points.map((p) => point(transform(p))),
							true,
						),
					);
				commands.push(entity.kind === "venue" ? "0.38 G" : "0.12 G", "0.65 w");
				for (const outline of geometry.outlines)
					commands.push(
						path(
							outline.map((p) => point(transform(p))),
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
				}
			}
			commands.push(
				"Q",
				"0.08 G",
				"0.9 w",
				`${n(BORDER)} ${n(BORDER)} ${n(W - BORDER * 2)} ${n(H - BORDER * 2)} re S`,
				`${n(BORDER)} ${n(drawingBottom)} ${n(W - BORDER * 2)} ${n(H - drawingBottom - BORDER)} re S`,
			);
			const tx = W - BORDER - 330;
			commands.push(
				"1 1 1 rg",
				`${n(tx)} ${n(BORDER)} 330 ${n(TITLE_H)} re f`,
				"0.08 G",
				`${n(tx)} ${n(BORDER)} 330 ${n(TITLE_H)} re S`,
				`${n(tx + 70)} ${n(BORDER)} 0 ${n(TITLE_H)} re S`,
				`${n(tx + 70)} ${n(BORDER + TITLE_H / 2)} 260 0 re S`,
				...mark(tx + 12, BORDER + 18),
			);
			commands.push(
				text("ToskLight Previs", tx + 76, BORDER + TITLE_H - 18, 10, true),
				text(
					info.showFileName || "Untitled show",
					tx + 76,
					BORDER + TITLE_H - 34,
					8,
					true,
				),
				text(
					`Lighting designer  ${info.lightingDesigner || "-"}`,
					tx + 76,
					BORDER + 29,
					7,
				),
				text(`Version  ${info.showVersion || "-"}`, tx + 76, BORDER + 17, 7),
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
			return commands.join("\n");
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
function pdfDocument(streams: readonly string[]) {
	const objects: string[] = [];
	const ids = streams.map((_, index) => 3 + index * 2);
	objects.push(
		"<< /Type /Catalog /Pages 2 0 R >>",
		`<< /Type /Pages /Count ${streams.length} /Kids [${ids.map((id) => `${id} 0 R`).join(" ")}] >>`,
	);
	for (let i = 0; i < streams.length; i++) {
		const contentId = ids[i] + 1;
		objects.push(
			`<< /Type /Page /Parent 2 0 R /MediaBox [0 0 ${n(W)} ${n(H)}] /Resources << /Font << /F1 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> /F2 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold >> >> >> /Contents ${contentId} 0 R >>`,
			`<< /Length ${new TextEncoder().encode(streams[i]).length} >>\nstream\n${streams[i]}\nendstream`,
		);
	}
	let output = "%PDF-1.4\n%ToskLight\n";
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
