/** Helvetica title-block fields fit their column while retaining complete searchable text. */
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
export function titleText(
	value: string,
	x: number,
	y: number,
	size: number,
	width: number,
	bold = false,
): string {
	const advance =
		Array.from(value).reduce(
			(sum, character) =>
				sum + (ADVANCES[character.charCodeAt(0) - 32] ?? 1015),
			0,
		) / 1000;
	// Bold's wider lower-case letters remain inside the same column; the margin also avoids
	// touching a column divider after PDF coordinate rounding.
	const fitted = Math.min(
		size,
		width / Math.max(0.001, advance * (bold ? 1.25 : 1)),
	);
	return text(value, x, y, Math.floor(fitted * 1000) / 1000, bold);
}
