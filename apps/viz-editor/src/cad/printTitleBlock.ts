/** Shared title furniture in PDF points, relative to the title box's lower-left corner. */
import {
	type CadPrintDocumentInfo, printDocumentSubtitle,
	printPageTitle, printScaleDenominator, printGridMillimetres, TITLE_H, TITLE_W,
} from "./printLayout";
import { saved, distance } from "./printPdfOps";
import type { CadPrintPage } from "./types";

export const PRINT_TITLE_MARK = { x: 252, y: 10, size: 36 } as const;
export const PRINT_TITLE_DIVIDER_X = 219;


export interface PrintTitleEntry {
	value: string;
	x: number;
	y: number;
	size: number;
	width: number;
	bold?: boolean;
	muted?: boolean;
	rotation?: 90;
}

function printTitleHeadings(page: CadPrintPage, info: CadPrintDocumentInfo) {
	const title = printPageTitle(page);
	const headings = [
		{ value: title || info.project || info.showName || "Lighting plan", size: 12, bold: true },
		{ value: title ? printDocumentSubtitle(info) : "", size: 8 },
		{ value: info.venue, size: 6 },
	].filter((entry) => entry.value).map((entry, index) => ({ ...entry, x: 22, y: TITLE_H - 13 - index * 11, width: 191 }));
	return headings;
}

export function printTitleSeparatorY(page: CadPrintPage, info: CadPrintDocumentInfo) {
	return TITLE_H - Math.max(28, 18 + (printTitleHeadings(page, info).length - 1) * 11);
}

export function printTitleEntries(page: CadPrintPage, info: CadPrintDocumentInfo): PrintTitleEntry[] {
	const headings = printTitleHeadings(page, info);
	const separator = printTitleSeparatorY(page, info);
	const contacts = [
		info.lightingDesigner,
		info.contactPhone ? `Phone: ${info.contactPhone}` : "",
		info.contactEmail ? `Email: ${info.contactEmail}` : "",
	].filter(Boolean).map((value, index) => ({ value, x: 22, y: separator - 9 - index * 9, size: 6, width: 191 }));
	const metadata = [
		`Printed  ${saved(info.printedAt ?? Date.now() / 1000)}`,
		`${info.fixtureCount} fixtures / ${info.universeCount} universes`,
		`Scale 1:${printScaleDenominator(page)} / Grid ${distance(printGridMillimetres(page))}`,
	].map((value, index) => ({ value, x: 225, y: TITLE_H - 12 - index * 9, size: 6, width: TITLE_W - 231 }));
	return [...headings, ...contacts, ...metadata, {
		value: "Project", x: 11, y: (separator + TITLE_H) / 2,
		size: 6, width: TITLE_H - separator - 8, rotation: 90,
	}, {
		value: "Designer", x: 11, y: separator / 2,
		size: 6, width: separator - 8, rotation: 90,
	}];
}

export const PRINT_GENERATED_FOOTER = "Generated with ToskLight Architect · www.tosklight.de";
