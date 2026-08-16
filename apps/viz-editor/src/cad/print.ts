import { entityPlanGeometry, type PlanPoint } from "./projection";
import {
	type CadPrintPage,
	type CadSceneSnapshot,
	printPageHeight,
	projectPoint,
} from "./types";

const PAPER_WIDTH_POINTS = 841.89;
const PAPER_HEIGHT_POINTS = 595.28;

export function buildCadPdf(
	scene: CadSceneSnapshot,
	pages: readonly CadPrintPage[],
): Uint8Array {
	const drawings = new Map(
		scene.drawings.map((drawing) => [drawing.id, drawing]),
	);
	const streams = pages.map((page) => {
		const height = printPageHeight(page);
		const scale = PAPER_WIDTH_POINTS / page.widthMillimetres;
		const point = (value: PlanPoint): PlanPoint => [
			(value[0] - page.centreMillimetres[0] + page.widthMillimetres / 2) *
				scale,
			(value[1] - page.centreMillimetres[1] + height / 2) * scale,
		];
		const commands = [
			"q",
			`0 0 ${number(PAPER_WIDTH_POINTS)} ${number(PAPER_HEIGHT_POINTS)} re W n`,
			"1 1 1 rg",
			`0 0 ${number(PAPER_WIDTH_POINTS)} ${number(PAPER_HEIGHT_POINTS)} re f`,
		];
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
			commands.push(entity.kind === "venue" ? "0.78 g" : "0.64 g");
			for (const triangle of geometry.triangles) {
				const [a, b, c] = triangle.points.map((value) =>
					point(transform(value)),
				);
				commands.push(path([a, b, c], true));
			}
			commands.push(entity.kind === "venue" ? "0.42 G" : "0.18 G", "0.65 w");
			for (const outline of geometry.outlines) {
				commands.push(
					path(
						outline.map((value) => point(transform(value))),
						false,
					),
				);
			}
			if (entity.kind !== "venue") {
				const centre = projectPoint(
					entity.positionMillimetres,
					page.view,
					page.rotationQuarterTurns,
				);
				const direction = projectPoint(
					entity.outputDirection.map((value) => value * 420) as [
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
		commands.push("Q");
		return commands.join("\n");
	});
	return pdfDocument(streams);
}

function path(
	points: readonly PlanPoint[],
	fill: boolean,
	close = true,
): string {
	if (!points.length) return "";
	const commands = [`${number(points[0][0])} ${number(points[0][1])} m`];
	for (const point of points.slice(1))
		commands.push(`${number(point[0])} ${number(point[1])} l`);
	if (close) commands.push("h");
	commands.push(fill ? "f" : "S");
	return commands.join(" ");
}

function number(value: number) {
	return Number.isFinite(value) ? value.toFixed(2).replace(/\.00$/, "") : "0";
}

function pdfDocument(streams: readonly string[]): Uint8Array {
	const objects: string[] = [];
	const pageObjectIds = streams.map((_, index) => 3 + index * 2);
	objects.push("<< /Type /Catalog /Pages 2 0 R >>");
	objects.push(
		`<< /Type /Pages /Count ${streams.length} /Kids [${pageObjectIds.map((id) => `${id} 0 R`).join(" ")}] >>`,
	);
	for (let index = 0; index < streams.length; index++) {
		const contentId = pageObjectIds[index] + 1;
		objects.push(
			`<< /Type /Page /Parent 2 0 R /MediaBox [0 0 ${number(PAPER_WIDTH_POINTS)} ${number(PAPER_HEIGHT_POINTS)}] /Resources << >> /Contents ${contentId} 0 R >>`,
		);
		objects.push(
			`<< /Length ${new TextEncoder().encode(streams[index]).length} >>\nstream\n${streams[index]}\nendstream`,
		);
	}
	let output = "%PDF-1.4\n%ToskLight\n";
	const offsets = [0];
	for (let index = 0; index < objects.length; index++) {
		offsets.push(new TextEncoder().encode(output).length);
		output += `${index + 1} 0 obj\n${objects[index]}\nendobj\n`;
	}
	const xref = new TextEncoder().encode(output).length;
	output += `xref\n0 ${objects.length + 1}\n0000000000 65535 f \n`;
	for (const offset of offsets.slice(1))
		output += `${String(offset).padStart(10, "0")} 00000 n \n`;
	output += `trailer\n<< /Size ${objects.length + 1} /Root 1 0 R >>\nstartxref\n${xref}\n%%EOF\n`;
	return new TextEncoder().encode(output);
}
