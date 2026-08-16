import type { SoftwareKey } from "@tosklight/ui/programmer-keypad";
import type { CommandTargetMode } from "../../controlSurface/commandTarget";
import { removeCommandToken } from "./commandLineEditing";

export type { SoftwareKey } from "@tosklight/ui/programmer-keypad";
export type { CommandTargetMode } from "../../controlSurface/commandTarget";
export {
	commandTargetAfterEnter,
	defaultCommandLine,
} from "../../controlSurface/commandTarget";

export const softwareKeypadRows: SoftwareKey[][] = [
	["TIME", "DIV", "-", "+"],
	["7", "8", "9", "AT"],
	["4", "5", "6", "TRU"],
	["1", "2", "3", "CLR"],
	["0", "BACKSPACE", ".", "ENT"],
];

export interface TargetedCommandEdit {
	command: string;
	execute: boolean;
	pristine: boolean;
}

const pristineRootTokens: Partial<Record<SoftwareKey, string>> = {
	GRP: "GROUP",
	CUE: "CUE",
	PLAYBACK: "PLAYBACK",
	OFF: "OFF",
	DEL: "DELETE",
	MOV: "MOVE",
	CPY: "COPY",
	SET: "SET",
	AT: "AT",
	TIME: "TIME",
	LINK: "LINK",
	SELECT: "SELECT",
	"+": "+",
	"-": "-",
	".": ".",
};

function shortTarget(target: CommandTargetMode): "F" | "G" {
	return target === "FIXTURE" ? "F" : "G";
}

function editPristineCommand(
	key: SoftwareKey,
	target: CommandTargetMode,
): TargetedCommandEdit | null {
	if (/^\d$/.test(key))
		return {
			command: `${shortTarget(target)}${key}`,
			execute: false,
			pristine: false,
		};
	if (key === "GRP")
		return {
			command: target === "GROUP" ? "FIXTURE" : "GROUP",
			execute: false,
			pristine: false,
		};
	const command = pristineRootTokens[key];
	return command ? { command, execute: false, pristine: false } : null;
}

export function softwareKeyFromKeyboard(
	event: Pick<KeyboardEvent, "code" | "key" | "shiftKey">,
	regularNumbers: boolean,
): SoftwareKey | null {
	if (/^Numpad\d$/.test(event.code)) return event.code.slice(-1) as SoftwareKey;
	if (regularNumbers && /^Digit\d$/.test(event.code) && !event.shiftKey)
		return event.code.slice(-1) as SoftwareKey;
	if (
		event.code === "NumpadDecimal" ||
		(event.code === "Period" && !event.shiftKey)
	)
		return ".";
	if (event.code === "NumpadAdd") return "+";
	if (event.code === "NumpadSubtract") return "-";
	if (event.code === "Escape") return "ESC";
	if (event.code === "Backspace") return "BACKSPACE";
	if (event.code === "Enter" || event.code === "NumpadEnter") return "ENT";
	if (event.code === "Delete") return "CLR";
	if (event.code === "Home") return "SET";
	if (event.code === "End") return "REC";
	// Physical positions on a German keyboard. Using code keeps the shortcuts
	// stable when the browser reports localized glyphs. Shift is exclusively
	// the command line's semantic second layer, so shifted layout shortcuts are
	// intentionally not mapped to regular desk commands.
	if (event.code === "BracketRight") return "+";
	if (event.code === "Minus") return "TRU";
	if (event.code === "Backquote") return "PRE";
	if (event.code === "Equal") return "DIV";
	if (event.code === "Backslash") return "AT";

	// Fallbacks help browsers that do not expose a useful physical key code.
	if (!event.shiftKey && event.key === "+") return "+";
	if (!event.shiftKey && event.key === "ß") return "TRU";
	if (!event.shiftKey && event.key === "^") return "PRE";
	if (!event.shiftKey && event.key === "´") return "DIV";
	if (!event.shiftKey && event.key === "#") return "AT";
	return null;
}

export function editTargetedCommandWithSoftwareKey(
	command: string,
	key: SoftwareKey,
	target: CommandTargetMode,
	pristine: boolean,
	repeated: boolean | undefined = undefined,
): TargetedCommandEdit {
	if (key === "BACKSPACE") {
		if (pristine) return { command: target, execute: false, pristine: true };
		const next = removeCommandToken(command);
		return next
			? { command: next, execute: false, pristine: false }
			: { command: target, execute: false, pristine: true };
	}
	if (key === "SHIFT") return { command, execute: false, pristine };

	const freeze = command.match(/^\s*(FREEZE|UNFREEZE)\b\s*(.*)$/i);
	if (freeze) {
		const prefix = freeze[1].toUpperCase();
		const selection = freeze[2].trim();
		const family = selection.match(
			/(?:^|\s)(INTENSITY|COL(?:OR|OUR)|POSITION|BEAM)(?:\s|$)/i,
	);
		const selectionText = family
			? selection.slice(0, family.index).trim()
			: selection;
		const familyText = family
			? selection.slice(family.index).trim()
			: "";
		const edited = editTargetedCommandWithSoftwareKey(
			selectionText || target,
			key,
			target,
			selectionText.length === 0,
		);
		return {
			...edited,
			command: `${prefix} ${edited.command}${familyText ? ` ${familyText}` : ""}`.trim(),
		};
	}

	if (pristine) {
		const edit = editPristineCommand(key, target);
		if (edit) return edit;
	}

	const selectionCommand =
		/^\s*(?:F\d|G\d|FIXTURE\b|GROUP\b|DEGROUP\b|DEGRP\b)/i.test(command);
	if (key === "GRP" && selectionCommand && /(?:\+|-)\s*$/.test(command)) {
		const override = target === "GROUP" ? "F" : "G";
		return {
			command: `${command.trimEnd()} ${override}`,
			execute: false,
			pristine: false,
		};
	}
	if (repeated !== false && key === "GRP" && /(?:^|\s)(?:GROUP|G|F)\s*$/i.test(command)) {
		return {
			command: command.replace(/(?:GROUP|G|F)\s*$/i, "DEGROUP"),
			execute: false,
			pristine: false,
		};
	}
	if (repeated !== false && key === "AT" && /(?:^|\s)AT\s*$/i.test(command)) {
		return {
			command: command.replace(/AT\s*$/i, "AT 100"),
			execute: true,
			pristine: false,
		};
	}
	if (key === "." && /^\s*SPD\s+GRP\b/i.test(command)) {
		return { command: `${command},`, execute: false, pristine: false };
	}
	if (repeated !== false && key === "." && /\.\s*$/.test(command)) {
		return {
			command: `${command.replace(/\.\s*$/, "").trimEnd()} AT 0`.trim(),
			execute: true,
			pristine: false,
		};
	}
	if (repeated !== false && key === "TIME" && /(?:^|\s)TIME\s*$/i.test(command)) {
		return {
			command: command.replace(/TIME\s*$/i, "DELAY"),
			execute: false,
			pristine: false,
		};
	}
	if (repeated !== false && key === "DIV" && /(?:^|\s)DIV\s*$/i.test(command)) {
		return {
			command: command.replace(/DIV\s*$/i, "OFFSET"),
			execute: false,
			pristine: false,
		};
	}
	const token =
		(
			{
				GRP: "GROUP",
				CUE: "CUE",
				PLAYBACK: "PLAYBACK",
				OFF: "OFF",
				DEL: "DELETE",
				MOV: "MOVE",
				CPY: "COPY",
				TRU: "THRU",
				DIV: "DIV",
				DIFF: "DIFF",
				SET: "SET",
				AT: "AT",
				TIME: "TIME",
				LINK: "LINK",
				SELECT: "SELECT",
				"+": "+",
				"-": "-",
			} as Partial<Record<SoftwareKey, string>>
		)[key] ?? key;
	const spaced = [
		"GROUP",
		"CUE",
		"DELETE",
		"MOVE",
		"COPY",
		"THRU",
		"DIV",
		"SET",
		"AT",
		"TIME",
		"LINK",
		"SELECT",
		"+",
		"-",
	].includes(token);
	if (/^\d$/.test(token) && /^\s*(?:GROUP|FIXTURE)\s*$/i.test(command)) {
		return {
			command: `${/^\s*GROUP/i.test(command) ? "G" : "F"}${token}`,
			execute: false,
			pristine: false,
		};
	}
	const selectionContinuation =
		(selectionCommand || /^\s*\+\s*$/.test(command)) &&
		/(?:\+|-)\s*$/.test(command) &&
		!/\bAT\b[\s\S]*$/i.test(command);
	const shortPrefixAwaitingNumber =
		/^\d$/.test(token) && /(?:^|\s)[FG]$/i.test(command);
	const digitAfterWord =
		/^\d$/.test(token) && /(?:[A-EH-Z]|[+-])\s*$/i.test(command);
	const nextToken =
		/^\d$/.test(token) && selectionContinuation
			? `${shortTarget(target)}${token}`
			: shortPrefixAwaitingNumber
				? token
				: digitAfterWord
					? ` ${token}`
					: token;
	const digitContinuation =
		/^\d$/.test(token) && selectionContinuation && command.trim() !== "+";
	return {
		// Command-line spaces are cosmetic separators only; the text never carries a
		// trailing space.
		command:
			`${command}${digitContinuation || spaced ? ` ${nextToken}` : nextToken}`
				.replace(/\s+/g, " ")
				.trim(),
		execute: false,
		pristine: false,
	};
}

export function editCommandWithSoftwareKey(command: string, key: SoftwareKey) {
	const { pristine: _pristine, ...edit } = editTargetedCommandWithSoftwareKey(
		command,
		key,
		"FIXTURE",
		false,
	);
	return edit;
}
