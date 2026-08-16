import type { SoftwareKey } from "../softwareKeypad";

export type CommandKeyGestureKind = "regular" | "double" | "hold";

export interface CommandKeyGesture {
	kind: CommandKeyGestureKind;
	shifted: boolean;
}

export type CommandKeyGestureIntent =
	| { type: "command"; text: string; replace: readonly string[] }
	| {
			type: "action";
			action:
				| "align-off"
				| "clear-preload"
				| "inspect-fixtures"
				| "inspect-groups"
				| "inspect-preload"
				| "lock"
				| "record-options"
				| "running-output"
				| "undo"
				| "update-options";
	  }
	| null;

const shiftedRoots: Partial<Record<SoftwareKey, string>> = {
	"0": "ALL",
	"1": "INTENSITY",
	"2": "COLOR",
	"3": "POSITION",
	"4": "BEAM",
	"5": "DYNAMICS",
	"6": "SHAPERS",
	"7": "FOCUS",
	"8": "CONTROL",
	"9": "MEDIA",
	AT: "FixAT",
	GRP: "FIXTURE",
	CUE: "TIMECODE",
	PLAYBACK: "MACRO",
	SET: "ASSIGN",
	TIME: "SPD GRP",
	DIV: "GO TO",
	OFF: "RELEASE",
	MOV: "COPY",
	REC: "UPDATE",
	CLR: "FREEZE",
};

const doubleRoots: Partial<Record<SoftwareKey, { text: string; replace: readonly string[] }>> = {
	GRP: { text: "DEGROUP", replace: ["GROUP", "FIXTURE"] },
	CUE: { text: "CUELIST", replace: ["CUE"] },
	PLAYBACK: { text: "VPBK", replace: ["PLAYBACK"] },
};

export function resolveCommandKeyGesture(
	key: SoftwareKey,
	gesture: CommandKeyGesture,
): CommandKeyGestureIntent {
	if (gesture.kind === "hold") {
		if (key === "GRP")
			return {
				type: "action",
				action: gesture.shifted ? "inspect-fixtures" : "inspect-groups",
			};
		if (key === "REC")
			return {
				type: "action",
				action: gesture.shifted ? "update-options" : "record-options",
			};
		if (key === "PRE") return { type: "action", action: "inspect-preload" };
		return null;
	}

	if (!gesture.shifted) {
		if (gesture.kind === "double") {
			if (key === "OFF") return { type: "action", action: "running-output" };
			const root = doubleRoots[key];
			return root ? { type: "command", ...root } : null;
		}
		return null;
	}

	if (gesture.kind === "double") {
		if (/^[0-9]$/.test(key)) {
			const family = shiftedRoots[key];
			return family
				? { type: "command", text: `${family} PRESET`, replace: [family] }
				: null;
		}
		if (key === "GRP")
			return { type: "command", text: "DMX", replace: ["FIXTURE"] };
		if (key === "DIV")
			return { type: "command", text: "LOAD", replace: ["GO TO"] };
		if (key === "CLR")
			return { type: "command", text: "UNFREEZE", replace: ["FREEZE"] };
	}

	if (key === "ENT") return { type: "action", action: "lock" };
	if (key === "ESC") return { type: "action", action: "undo" };
	if (key === "PRE") return { type: "action", action: "clear-preload" };
	if (key === "ALIGN") return { type: "action", action: "align-off" };
	if (key === "CLR")
		return { type: "command", text: "FREEZE", replace: ["*"] };
	const root = shiftedRoots[key];
	return root ? { type: "command", text: root, replace: [] } : null;
}

export function applyGestureCommand(current: string, pristine: boolean, intent: Extract<CommandKeyGestureIntent, { type: "command" }>) {
	const text = current.trim();
	if (intent.replace.includes("*")) return intent.text;
	for (const replace of intent.replace) {
		if (text.toLocaleUpperCase().endsWith(replace.toLocaleUpperCase()))
			return `${text.slice(0, -replace.length).trimEnd()} ${intent.text}`.trim();
	}
	return pristine || text === "FIXTURE" || text === "GROUP"
		? intent.text
		: `${text} ${intent.text}`;
}
