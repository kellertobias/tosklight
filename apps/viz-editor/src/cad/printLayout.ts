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
export const PRINT_TITLE_HEIGHT_MM = 24;
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
	/** The lighting designer's company logo as the show stores it; printed in place of the mark. */
	companyLogo?: string;
	lastSavedAt: number;
	/** Export time in Unix seconds; sampled once for the complete PDF when omitted. */
	printedAt?: number;
	fixtureCount: number;
	universeCount: number;
}

/** Physical drawing width after reserving the border and full-width title band. */
function fittedDrawingWidth(page: Pick<CadPrintPage, "orientation">) {
	const paper = printPaperSize(page);
	return Math.min(
		paper.width - PRINT_BORDER_MM * 2,
		(paper.height - PRINT_BORDER_MM * 2 - PRINT_TITLE_HEIGHT_MM) * paper.width / paper.height,
	);
}

/** Sheet millimetres per plan millimetre, shared by geometry and the scale legend. */
export function printPageScale(page: Pick<CadPrintPage, "widthMillimetres" | "orientation">) {
	return fittedDrawingWidth(page) / page.widthMillimetres;
}

export function printScaleDenominator(page: Pick<CadPrintPage, "widthMillimetres" | "orientation">) {
	return Math.max(1, Math.round(1 / printPageScale(page)));
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
	const oldWidth = fittedDrawingWidth(page);
	const orientation =
		page.orientation === "portrait" ? "landscape" : "portrait";
	const nextWidth = fittedDrawingWidth({ orientation });
	return {
		...page,
		orientation,
		widthMillimetres:
			page.widthMillimetres * (nextWidth / oldWidth),
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

/** Default numbered page names identify a sheet; the heading then uses the project or show name. */
export function printPageTitle(page: Pick<CadPrintPage, "name" | "view">) {
	const name = page.name.trim();
	return name && !/^Page\s*\d+$/i.test(name) ? name : "";
}

export function printDocumentSubtitle(info: CadPrintDocumentInfo) {
	return [info.project.trim(), info.showName.trim()]
		.filter((value, index, values) => value && values.indexOf(value) === index)
		.join(" / ");
}

export function printDocumentDetails(info: CadPrintDocumentInfo) {
	return [info.lightingDesigner, info.venue].filter(Boolean).join(" / ");
}
