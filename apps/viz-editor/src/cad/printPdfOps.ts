/**
 * The PDF operators this app writes by hand: numbers, paths, text, the application mark, and the
 * document that carries the page streams. No PDF library is involved, so every helper here emits
 * content-stream syntax directly.
 */
import architectIconSvg from "../../../../assets/branding/tosklight-icon-print.svg?raw";
import type { PlanPoint } from "./projection";

export function mark(x: number, y: number) {
	const cyanPath = svgIconPath("rgb(0,182,255)");
	const whitePath = svgIconPath("white");
	const scale = 50 / 1024;
	return [
		"0.01 0.03 0.08 rg",
		`${n(x)} ${n(y - 2)} 52 52 re f`,
		"q",
		`${n(scale)} 0 0 ${n(-scale)} ${n(x + 1)} ${n(y + 49)} cm`,
		"0.02 0.71 1 rg",
		"1.1301 0 0 1.10327 -91.1666 -52.8747 cm",
		"2.27038 0 0 2.27038 479.14 600.211 cm",
		`${cyanPath} f`,
		"Q",
		"q",
		`${n(scale)} 0 0 ${n(-scale)} ${n(x + 1)} ${n(y + 49)} cm`,
		"1 1 1 rg",
		"1.1301 0 0 1.10327 -91.1666 -52.8747 cm",
		"2.27038 0 0 2.27038 489.721 673.347 cm",
		`${whitePath} f`,
		"Q",
	];
}

export function svgIconPath(fill: string) {
	const matches = [
		...architectIconSvg.matchAll(/<path d="([^"]+)" style="fill:([^;]+);"/g),
	]
		.filter((match) => match[2] === fill)
		.sort((left, right) => right[1].length - left[1].length);
	if (!matches[0])
		throw new Error(
			`ToskLight application SVG is missing its ${fill} vector path`,
		);
	const tokens =
		matches[0][1].match(/[MLCZ]|-?(?:\d+\.?\d*|\.\d+)(?:e[-+]?\d+)?/gi) ?? [];
	const output: string[] = [];
	for (let index = 0; index < tokens.length; ) {
		const command = tokens[index++].toUpperCase();
		const take = (count: number) =>
			tokens.slice(index, (index += count)).map(Number);
		if (command === "M" || command === "L") {
			const [px, py] = take(2);
			output.push(`${n(px)} ${n(py)} ${command === "M" ? "m" : "l"}`);
		} else if (command === "C") {
			const values = take(6);
			output.push(`${values.map(n).join(" ")} c`);
		} else if (command === "Z") output.push("h");
		else
			throw new Error(
				`Unsupported command ${command} in ToskLight application SVG`,
			);
	}
	return output.join(" ");
}
export function saved(seconds: number) {
	return seconds
		? `${new Date(seconds * 1000).toISOString().slice(0, 16).replace("T", " ")} UTC`
		: "-";
}
export function distance(mm: number) {
	return mm >= 1000 ? `${n(mm / 1000)} m` : `${n(mm)} mm`;
}
export function text(value: string, x: number, y: number, size: number, bold = false) {
	return `BT /${bold ? "F2" : "F1"} ${size} Tf ${n(x)} ${n(y)} Td (${value.replace(/[^\x20-\xff]/g, "-").replace(/([\\()])/g, "\\$1")}) Tj ET`;
}
export function path(points: readonly PlanPoint[], fill: boolean, close = true) {
	if (!points.length) return "";
	const result = [`${n(points[0][0])} ${n(points[0][1])} m`];
	for (const p of points.slice(1)) result.push(`${n(p[0])} ${n(p[1])} l`);
	if (close) result.push("h");
	result.push(fill ? "f" : "S");
	return result.join(" ");
}
export function n(value: number) {
	return Number.isFinite(value) ? value.toFixed(2).replace(/\.00$/, "") : "0";
}
export function pdfDocument(
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
	let output = "%PDF-1.4\n%ToskLight Architect\n";
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
