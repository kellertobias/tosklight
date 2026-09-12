/**
 * Sheet geometry shared by everything that draws a print page: the paper, its border and title
 * box in millimetres and PDF points, the scale the page prints at, and its grid.
 */
import {
	type CadPrintPage,
	printPaperSize,
} from "./types";

export const PT_MM = 72 / 25.4;
export const PRINT_BORDER_MM = 7;
export const PRINT_TITLE_WIDTH_MM = 116.42;
export const PRINT_TITLE_HEIGHT_MM = 30;
export const BORDER = PRINT_BORDER_MM * PT_MM;
export const TITLE_H = PRINT_TITLE_HEIGHT_MM * PT_MM;
export const TITLE_W = PRINT_TITLE_WIDTH_MM * PT_MM;

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
