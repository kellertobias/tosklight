export type HighlightControlAction =
	| "on"
	| "off"
	| "toggle"
	| "next"
	| "previous"
	| "all";

export type AttachedHighlightAction = Extract<
	HighlightControlAction,
	"toggle" | "next" | "previous" | "all"
>;

export type EncoderControlAction = "up" | "down" | "left" | "right" | "press";

export const encoderControlActions: readonly EncoderControlAction[] = [
	"up",
	"down",
	"left",
	"right",
	"press",
];

export type ProgrammerControlAction =
	| `digit-${0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9}`
	| "at"
	| "align"
	| "backspace"
	| "clear"
	| "cpy"
	| "cue"
	| "delay"
	| "diff"
	| "del"
	| "div"
	| "dot"
	| "enter"
	| "escape"
	| "group"
	| "link"
	| "menu"
	| "minus"
	| "mov"
	| "off"
	| "playback"
	| "page-up"
	| "page-down"
	| "plus"
	| "preload"
	| "prog-playback"
	| "record"
	| "select"
	| "set"
	| "shift"
	| "thru"
	| "time"
	| "undo";

export type PlaybackControl = "fader" | `button/${1 | 2 | 3}`;

/** Canonical relative OSC paths after the authenticated `/light/{desk}/` prefix. */
export const controlSurfaceOscPaths = {
	page: "page",
	explicitPlayback: (page: number, position: number) =>
		`playback/${page}/${position}`,
	explicitPlaybackControl: (
		page: number,
		position: number,
		control: PlaybackControl,
	) => `playback/${page}/${position}/${control}`,
	programmer: (action: ProgrammerControlAction) => `programmer/${action}`,
	programmerFade: (kind: "programmer" | "cue" | "release") =>
		`programmer/${kind === "programmer" ? "prog" : kind}-fade`,
	highlight: (action: HighlightControlAction) => `highlight/${action}`,
	speedGroupButton: (group: number) => `speed-group/${group}/button`,
	speedGroupEncoder: (group: number) => `speed-group/${group}/encoder`,
	encoder: (number: number) => `encode/${number}`,
	navigation: "nav",
} as const;

export function feedbackPlaybackOffset(parts: readonly string[]): number {
	const offset = parts.indexOf("playback");
	return offset > 0 && parts[offset - 1] === "feedback" ? offset : -1;
}

export const attachedHighlightKeys: readonly {
	label: "HIGH" | "PREV" | "NEXT" | "ALL";
	action: AttachedHighlightAction;
	column: number;
	row: number;
}[] = [
	{ label: "HIGH", action: "toggle", column: 1, row: 1 },
	{ label: "PREV", action: "previous", column: 2, row: 1 },
	{ label: "NEXT", action: "next", column: 3, row: 1 },
	{ label: "ALL", action: "all", column: 4, row: 1 },
];

export const attachedProgrammerActionLayout = {
	record: { column: 1, columnSpan: 2, row: 1, rowSpan: 1 },
	preload: { column: 3, columnSpan: 2, row: 1, rowSpan: 1 },
} as const;

export const attachedKeypadContentRowOffset = 1;

const inclusiveRange = (first: number, last: number): readonly number[] =>
	Array.from({ length: last - first + 1 }, (_, index) => first + index);

export const attachedPlaybackLayout = {
	mainSlots: inclusiveRange(1, 20),
	topSlots: inclusiveRange(21, 40),
	gridButtonSlots: inclusiveRange(41, 90),
	gridPlaybackSlots: inclusiveRange(91, 96),
	encoderSlots: inclusiveRange(1, 6),
	navigationEncoder: 7,
	speedGroups: inclusiveRange(1, 5),
} as const;

/** Stable physical control identity for the fixed attached-simulator wiring, independent of page. */
export function attachedPlaybackControlId(
	position: number,
	control: PlaybackControl,
): number {
	return (
		position * 8 + (control === "fader" ? 6 : Number(control.split("/")[1]))
	);
}
