import type { ProgrammerControlAction } from "./controlSurfaceContracts";

export type DigitKey =
	| "0"
	| "1"
	| "2"
	| "3"
	| "4"
	| "5"
	| "6"
	| "7"
	| "8"
	| "9";

export type SoftwareKey =
	| "SET"
	| "GRP"
	| "CUE"
	| "PLAYBACK"
	| "OFF"
	| "UND"
	| "CLR"
	| "DEL"
	| "MOV"
	| "CPY"
	| "TRU"
	| "DIV"
	| "DIFF"
	| "PAGE_UP"
	| "PAGE_DOWN"
	| "BACKSPACE"
	| "AT"
	| "ENT"
	| "PRE"
	| "REC"
	| "ESC"
	| "SHIFT"
	| "TIME"
	| "LINK"
	| "SELECT"
	| "+"
	| "-"
	| "."
	| DigitKey;

export type NumericPadSection = "commands" | "numbers";

export interface NumericPadLayoutItem {
	key: SoftwareKey;
	section: NumericPadSection;
	column: number;
	row: number;
	rowSpan?: number;
}

// Shared physical layout for the software number block and attached/simulated desks.
// Number-section columns retain their full-surface positions so the gap between the
// two blocks remains explicit in layout tests and hardware renderers.
export const numericPadLayout: NumericPadLayoutItem[] = [
	{ key: "DEL", section: "commands", column: 1, row: 2 },
	{ key: "CLR", section: "commands", column: 2, row: 2 },
	{ key: "PLAYBACK", section: "commands", column: 3, row: 2 },
	{ key: "OFF", section: "commands", column: 4, row: 2 },
	{ key: "MOV", section: "commands", column: 1, row: 3 },
	{ key: "BACKSPACE", section: "commands", column: 2, row: 3 },
	{ key: "DIFF", section: "commands", column: 3, row: 3 },
	{ key: "ESC", section: "commands", column: 4, row: 3 },
	{ key: "CPY", section: "commands", column: 1, row: 4 },
	{ key: "UND", section: "commands", column: 2, row: 4 },
	{ key: "PAGE_UP", section: "commands", column: 3, row: 4 },
	{ key: "PAGE_DOWN", section: "commands", column: 4, row: 4 },
	{ key: "SET", section: "commands", column: 1, row: 5 },
	{ key: "SHIFT", section: "commands", column: 2, row: 5 },
	{ key: "GRP", section: "numbers", column: 4, row: 1 },
	{ key: "CUE", section: "numbers", column: 5, row: 1 },
	{ key: "TIME", section: "numbers", column: 6, row: 1 },
	{ key: "DIV", section: "numbers", column: 7, row: 1 },
	{ key: "7", section: "numbers", column: 4, row: 2 },
	{ key: "8", section: "numbers", column: 5, row: 2 },
	{ key: "9", section: "numbers", column: 6, row: 2 },
	{ key: "-", section: "numbers", column: 7, row: 2 },
	{ key: "4", section: "numbers", column: 4, row: 3 },
	{ key: "5", section: "numbers", column: 5, row: 3 },
	{ key: "6", section: "numbers", column: 6, row: 3 },
	{ key: "+", section: "numbers", column: 7, row: 3 },
	{ key: "1", section: "numbers", column: 4, row: 4 },
	{ key: "2", section: "numbers", column: 5, row: 4 },
	{ key: "3", section: "numbers", column: 6, row: 4 },
	{ key: "TRU", section: "numbers", column: 7, row: 4 },
	{ key: ".", section: "numbers", column: 4, row: 5 },
	{ key: "0", section: "numbers", column: 5, row: 5 },
	{ key: "AT", section: "numbers", column: 6, row: 5 },
	{ key: "ENT", section: "numbers", column: 7, row: 5 },
];

export type SoftwareDeskInputMode = "keyboard" | "touch";

/** Software-only desk geometry. The attached desk keeps `numericPadLayout`. */
export function softwareDeskKeypadLayout(
	mode: SoftwareDeskInputMode,
): NumericPadLayoutItem[] {
	const right: readonly SoftwareKey[] =
		mode === "keyboard"
			? ["DEL", "MOV", "CPY", "SET"]
			: ["DEL", "MOV", "SET", "SHIFT"];
	return [
		...(["GRP", "CUE", "PLAYBACK", "OFF"] as const).map((key, index) => ({
			key,
			section: "commands" as const,
			column: 1,
			row: index + 2,
		})),
		...right.map((key, index) => ({
			key,
			section: "commands" as const,
			column: 2,
			row: index + 2,
		})),
		...(
			[
				["TIME", "DIV", "-", "+"],
				["7", "8", "9", "AT"],
				["4", "5", "6", "TRU"],
				["1", "2", "3", "CLR"],
				["BACKSPACE", "0", ".", "ENT"],
			] as const
		).flatMap((row, rowIndex) =>
			row.map((key, columnIndex) => ({
				key,
				section: "numbers" as const,
				column: columnIndex + 4,
				row: rowIndex + 1,
			})),
		),
	];
}

const oscActionNames: Partial<Record<SoftwareKey, ProgrammerControlAction>> = {
	BACKSPACE: "backspace",
	ENT: "enter",
	GRP: "group",
	PLAYBACK: "playback",
	OFF: "off",
	DIFF: "diff",
	PAGE_UP: "page-up",
	PAGE_DOWN: "page-down",
	TRU: "thru",
	".": "dot",
	"+": "plus",
	"-": "minus",
	DEL: "del",
	MOV: "mov",
	CPY: "cpy",
	ESC: "escape",
	CLR: "clear",
	UND: "undo",
	REC: "record",
	PRE: "preload",
};

export function oscProgrammerActionForKey(
	key: SoftwareKey,
): ProgrammerControlAction {
	if (isDigitKey(key)) return `digit-${key}`;
	return oscActionNames[key] ?? (key.toLowerCase() as ProgrammerControlAction);
}

function isDigitKey(key: SoftwareKey): key is DigitKey {
	return /^\d$/.test(key);
}

export function softwareKeyLabel(key: SoftwareKey): string {
	if (key === "BACKSPACE") return "←";
	if (key === "PAGE_UP") return "PAGE ▲";
	if (key === "PAGE_DOWN") return "PAGE ▼";
	return key;
}
