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
	| "backspace"
	| "clear"
	| "cpy"
	| "cue"
	| "delay"
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

/**
 * Canonical relative OSC paths used after the `/light/{desk}/` prefix.
 *
 * `pagePlayback` follows the desk's current page. `explicitPlayback` deliberately omits the desk
 * alias because `/light/playback/{page}/{slot}` is global and never follows a page change.
 */
export const controlSurfaceOscPaths = {
	page: "page",
	pagePlayback: (slot: number) => `page-playback/${slot}`,
	pagePlaybackControl: (slot: number, control: PlaybackControl) =>
		`page-playback/${slot}/${control}`,
	explicitPlayback: (page: number, slot: number) => `playback/${page}/${slot}`,
	explicitPlaybackControl: (
		page: number,
		slot: number,
		control: PlaybackControl,
	) => `playback/${page}/${slot}/${control}`,
	programmer: (action: ProgrammerControlAction) => `programmer/${action}`,
	programmerFade: (kind: "programmer" | "cue") =>
		`programmer/${kind === "programmer" ? "prog" : "cue"}-fade`,
	highlight: (action: HighlightControlAction) => `highlight/${action}`,
	speedGroupButton: (group: number) => `speed-group/${group}/button`,
	speedGroupEncoder: (group: number) => `speed-group/${group}/encoder`,
	encoder: (number: number) => `encode/${number}`,
	navigation: "nav",
} as const;

export function feedbackPagePlaybackOffset(parts: readonly string[]): number {
	const canonical = parts.indexOf("page-playback");
	return canonical >= 0 ? canonical : parts.indexOf("paged-playback");
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
	record: { column: 1, row: 1, rowSpan: 2 },
	preload: { column: 2, row: 1, rowSpan: 2 },
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
