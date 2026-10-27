/** Shared title furniture in PDF points, relative to the title box's lower-left corner. */
import {
	type CadPrintDocumentInfo, printDocumentDetails, printDocumentSubtitle,
	printPageTitle, printScaleDenominator, TITLE_H, TITLE_W,
} from "./printLayout";
import { saved } from "./printPdfOps";
import type { CadPrintPage } from "./types";

export const PRINT_TITLE_MARK = { x: 6, y: 8, size: 52 } as const;
export const PRINT_TITLE_DIVIDER_X = 214;

export interface PrintTitleEntry {
	value: string;
	x: number;
	y: number;
	size: number;
	width: number;
	bold?: boolean;
}

export function printTitleEntries(page: CadPrintPage, info: CadPrintDocumentInfo): PrintTitleEntry[] {
	const title = printPageTitle(page);
	const main = [
		{ value: title || info.project || info.showName || "Lighting plan", y: TITLE_H - 14, size: 12, bold: true },
		{ value: title ? printDocumentSubtitle(info) : (info.project ? info.showName : ""), y: TITLE_H - 25, size: 8 },
		{ value: "Generated with ToskLight Architect", y: TITLE_H - 35, size: 6 },
		{ value: printDocumentDetails(info), y: 20, size: 6 },
		{ value: info.contactEmail, y: 9, size: 6 },
	].filter((entry) => entry.value).map((entry) => ({ ...entry, x: 64, width: 144 }));
	const metadata = [
		`Printed  ${saved(info.printedAt ?? Date.now() / 1000)}`,
		`${info.fixtureCount} fixtures / ${info.universeCount} universes`,
		`Scale 1:${printScaleDenominator(page)}`,
		info.contactPhone,
		info.showDate ? `Show date  ${info.showDate}` : "",
		info.showVersion ? `Version  ${info.showVersion}` : "",
	].filter(Boolean).map((value, index) => ({ value, x: 220, y: TITLE_H - 12 - index * 9, size: 6, width: TITLE_W - 226 }));
	return [...main, ...metadata];
}
