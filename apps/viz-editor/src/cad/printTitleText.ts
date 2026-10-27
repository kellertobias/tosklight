/** Helvetica title-block fields stay readable and ellipsize at the column boundary. */
import { text } from "./printPdfOps";

// Standard Helvetica advances, ASCII space through tilde, in 1/1000 em.
const ADVANCES = [
	278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278,
	278, 556, 556, 556, 556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584,
	584, 556, 1015, 667, 667, 722, 722, 667, 611, 778, 722, 278, 500, 667, 556,
	833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667, 667, 611, 278,
	278, 278, 469, 556, 333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222,
	500, 222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500,
	500, 334, 260, 334, 584,
];
export function fitTitleText(value: string, size: number, width: number, bold = false): { label: string; size: number } {
	const clean = value.replace(/\s+/g, " ").trim();
	const measure = (label: string) => Array.from(label).reduce(
		(sum, character) => sum + (ADVANCES[character.charCodeAt(0) - 32] ?? 1015),
		0,
	) / 1000 * (bold ? 1.25 : 1);
	// A conservative bold allowance also leaves breathing room beside a column divider.
	const minimum = Math.min(size, bold && size >= 10 ? 10 : 6);
	const fitted = Math.max(minimum, Math.min(size, width / Math.max(0.001, measure(clean))));
	let label = clean;
	if (measure(label) * fitted > width) {
		while (label && measure(`${label}...`) * fitted > width) label = label.slice(0, -1);
		label = label ? `${label}...` : "";
	}
	return { label, size: Math.floor(fitted * 1000) / 1000 };
}

export function titleText(value: string, x: number, y: number, size: number, width: number, bold = false): string {
	const fitted = fitTitleText(value, size, width, bold);
	return text(fitted.label, x, y, fitted.size, bold);
}
