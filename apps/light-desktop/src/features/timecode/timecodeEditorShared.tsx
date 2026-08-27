import { Button } from "@tosklight/ui";
import type { CSSProperties, RefObject } from "react";
import type { CueList } from "../../api/types";
import type { timelineItems } from "./editorModel";

export type TimelineItem = ReturnType<typeof timelineItems>[number];

export const TIMECODE_LANE_HEADER_WIDTH = 160;
export const TIMECODE_MARKER_COLORS = [
	{ name: "White", value: "#ffffff", text: "#11121a" },
	{ name: "Blue", value: "#58d4ef", text: "#11121a" },
	{ name: "Green", value: "#33aa77", text: "#11121a" },
	{ name: "Yellow", value: "#f5c451", text: "#11121a" },
	{ name: "Purple", value: "#a67cff", text: "#ffffff" },
	{ name: "Orange", value: "#f39a4a", text: "#11121a" },
] as const;

const DEFAULT_TIMECODE_MARKER_COLOR = TIMECODE_MARKER_COLORS[0];

export function markerColorOption(color?: string | null) {
	return (
		TIMECODE_MARKER_COLORS.find(
			(option) => option.value.toLowerCase() === color?.toLowerCase(),
		) ?? DEFAULT_TIMECODE_MARKER_COLOR
	);
}

export function markerColorIndex(color?: string | null): number {
	const index = TIMECODE_MARKER_COLORS.findIndex(
		(option) => option.value.toLowerCase() === color?.toLowerCase(),
	);
	return index < 0 ? 0 : index;
}

export function wrappedIndex(value: number, length: number): number {
	if (!length) return 0;
	return ((Math.round(value) % length) + length) % length;
}

export function timelineFrameX(frame: number, pixelsPerFrame: number): number {
	return TIMECODE_LANE_HEADER_WIDTH + frame * pixelsPerFrame;
}

export interface TimecodeCueListOption {
	id: string;
	name: string;
	/// The playback number the Cuelist is assigned to, when it has one. A Cuelist that is not in
	/// the playback pool is identified by name alone.
	number?: number;
	cues: readonly { id?: string; number: string; name: string }[];
	objectId?: string;
	revision?: number;
	body?: CueList;
}

export interface TimecodeAudioPlayerOption {
	fixtureId: string;
	name: string;
}

export function MarkerColorButton({
	color,
	onChange,
}: {
	color?: string | null;
	onChange(color: string): void;
}) {
	const selected = markerColorOption(color);
	const next = () => {
		const index = wrappedIndex(
			markerColorIndex(selected.value) + 1,
			TIMECODE_MARKER_COLORS.length,
		);
		const option = TIMECODE_MARKER_COLORS[index];
		if (option) onChange(option.value);
	};
	return (
		<Button
			className="timecode-keyframe-action timecode-marker-color-action"
			size="compact"
			aria-label={`Marker color: ${selected.name}. Select next color`}
			style={
				{
					"--timecode-marker-color": selected.value,
					"--timecode-marker-text-color": selected.text,
				} as CSSProperties
			}
			onClick={next}
		>
			{selected.name}
		</Button>
	);
}

export function formatFrame(frame: number, fps: number): string {
	const whole = Math.max(0, Math.round(frame));
	const seconds = Math.floor(whole / fps);
	return `${String(Math.floor(seconds / 3600)).padStart(2, "0")}:${String(Math.floor(seconds / 60) % 60).padStart(2, "0")}:${String(seconds % 60).padStart(2, "0")}.${String(whole % fps).padStart(2, "0")}`;
}

export function parseTimelineFrame(value: string, fps: number): number | null {
	const match = value.trim().match(/^(\d{1,2}):(\d{2}):(\d{2})[.:](\d{2})$/);
	if (!match) return null;
	const [, hours, minutes, seconds, frames] = match;
	const minute = Number(minutes);
	const second = Number(seconds);
	const frame = Number(frames);
	if (minute > 59 || second > 59 || frame >= fps) return null;
	return (Number(hours) * 60 * 60 + minute * 60 + second) * fps + frame;
}

/// The easing curves a Timecode keyframe may be given, in the order they are offered.
export const TIMECODE_EASINGS: Array<{
	value: "linear" | "ease_in" | "ease_out" | "ease_in_out";
	label: string;
}> = [
	{ value: "linear", label: "Linear" },
	{ value: "ease_in", label: "Ease in" },
	{ value: "ease_out", label: "Ease out" },
	{ value: "ease_in_out", label: "Ease in/out" },
];

/**
 * Moves the zoomed window, rather than only the number describing it.
 *
 * Setting the state alone would leave the timeline where it was, which is what an encoder driving
 * the window has to avoid.
 */
export function timelineScroller(
	scrollRef: RefObject<HTMLDivElement | null>,
	setScrollLeft: (value: number) => void,
) {
	return (next: number) => {
		if (scrollRef.current) scrollRef.current.scrollLeft = next;
		setScrollLeft(next);
	};
}

/** A pending overview-window resize, held while the operator is still dragging it. */
export interface OverviewResize {
	zoom: number;
	startFraction: number;
	revision: number;
}
