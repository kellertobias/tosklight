import type { DeskCommand } from "../features/controlSurfaceInteraction/registry";
import type { BuiltInWindow } from "../types";

export type BuiltInDestination = readonly [
	BuiltInWindow,
	icon: string,
	label: string,
];

export const builtIns: readonly BuiltInDestination[] = [
	["stage", "⌖", "Stage"],
	["fixtures", "♙", "Fixtures"],
	["presets", "▣", "Presets"],
	["cuelists", "▶", "Cuelists"],
	["dynamics", "∿", "Dynamics"],
	["channels", "▥", "Channels"],
];

export const shiftedBuiltIns: readonly BuiltInDestination[] = [
	["media", "▤", "Media"],
	["groups", "♟", "Groups"],
	["presets", "▣", "Presets"],
	["timecode", "◷", "Timecode"],
	["macros", "⚙", "Macros"],
	["dmx", "▥", "DMX"],
];

export function builtInsForShift(shiftHeld: boolean) {
	return shiftHeld ? shiftedBuiltIns : builtIns;
}

const normalDeskCommandWindows: Partial<Record<DeskCommand, BuiltInWindow>> = {
	stage: "stage",
	fixtures: "fixtures",
	groups: "groups",
	presets: "presets",
	cues: "cuelists",
	dynamics: "dynamics",
	channels: "channels",
	help: "help",
};

const shiftedDeskCommandWindows: Partial<Record<DeskCommand, BuiltInWindow>> = {
	stage: "media",
	fixtures: "groups",
	presets: "presets",
	cues: "timecode",
	dynamics: "macros",
	channels: "dmx",
};

export function builtInForDeskCommand(
	command: DeskCommand,
	shiftHeld: boolean,
): BuiltInWindow | undefined {
	return shiftHeld
		? (shiftedDeskCommandWindows[command] ?? normalDeskCommandWindows[command])
		: normalDeskCommandWindows[command];
}
