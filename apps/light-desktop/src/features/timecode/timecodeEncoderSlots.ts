import { useEffect } from "react";
import type { Cue, CueList } from "../../api/types";
import type { TimecodeDefinition } from "../../api/types/timecode";
import type { CueClipTimingDefaults } from "./cueClipTiming";
import {
	SPEED_GROUP_MAX_BPM,
	SPEED_GROUP_MIN_BPM,
} from "../speedGroupRuntime/contracts";
import { moveTimelineItem, type TimecodeEditorSelection } from "./editorModel";
import {
	formatFrame,
	markerColorIndex,
	markerColorOption,
	TIMECODE_MARKER_COLORS,
	type TimelineItem,
	wrappedIndex,
} from "./timecodeEditorShared";
import { publishTimecodeEncoderDeck } from "./timecodeEncoderBridge";

type Lane = TimecodeDefinition["lanes"][number];

/// Everything the encoder deck needs to edit the Cue timings a selected Cuelist clip drives.
export interface CueEncoderContext {
	/// The Cues the clip spans, in running order.
	cues: readonly { id?: string; number: string; name: string }[];
	selectedCueId: string | null;
	/// The Cuelist body the Cues live in, and the object id to save it under.
	cueListId: string;
	cueList: CueList;
	timingDefaults: CueClipTimingDefaults;
	setSelectedCueId(cueId: string | null): void;
	saveCueList(cueListId: string, body: CueList): Promise<CueList>;
}

interface EncoderSlotOptions {
	definition: TimecodeDefinition;
	cueContext?: CueEncoderContext;
	items: readonly TimelineItem[];
	keyframeItems: readonly TimelineItem[];
	selection: TimecodeEditorSelection | null;
	activeLane?: Lane;
	activeKeyframe?: TimelineItem;
	activeMarker?: TimecodeDefinition["markers"][number];
	speedKeyframe?: { bpm: number };
	volumeKeyframe?: { value: number };
	laneIndex: number;
	keyframeIndex: number;
	duration: number;
	frame: number;
	fps: number;
	zoom: number;
	maximumZoom: number;
	/// Where the zoomed window starts, in pixels, and how much of the timeline it can see.
	scrollLeft: number;
	viewportWidth: number;
	timelineWidth: number;
	encoderOwner: symbol;
	setZoom(value: number): void;
	setScrollLeft(value: number): void;
	setSelection(value: TimecodeEditorSelection | null): void;
	setSelectedLaneId(laneId: string): void;
	onScrub(frame: number): void;
	onCommit(value: TimecodeDefinition): void;
}

/// The longest Cue delay or fade an encoder will write, so a slip cannot store an absurd wait.
const MAXIMUM_CUE_TIMING_MILLIS = 10 * 60 * 1_000;

type CueCoreTiming = Pick<
	Cue,
	| "delay_millis"
	| "fade_millis"
	| "out_delay_millis"
	| "out_fade_millis"
	| "out_delay_link"
	| "out_fade_link"
>;

function clampIndex(value: number, length: number): number {
	if (length <= 0) return 0;
	return Math.max(0, Math.min(length - 1, Math.round(value)));
}

/// Writes one keyframe value back into its lane, clamped to that lane's own range.
export function laneWithKeyframeValue(
	definition: TimecodeDefinition,
	laneId: string,
	selection: TimecodeEditorSelection,
	value: number,
): TimecodeDefinition {
	return {
		...definition,
		lanes: definition.lanes.map((lane) => {
			if (lane.id !== laneId) return lane;
			if (selection.kind === "speed" && lane.content.kind === "speed_group")
				return {
					...lane,
					content: {
						...lane.content,
						keyframes: lane.content.keyframes.map((keyframe) =>
							keyframe.id !== selection.itemId
								? keyframe
								: {
										...keyframe,
										bpm: Math.max(
											SPEED_GROUP_MIN_BPM,
											Math.min(SPEED_GROUP_MAX_BPM, value),
										),
									},
						),
					},
				};
			if (selection.kind === "volume" && lane.content.kind === "audio_volume")
				return {
					...lane,
					content: {
						...lane.content,
						keyframes: lane.content.keyframes.map((keyframe) =>
							keyframe.id !== selection.itemId
								? keyframe
								: { ...keyframe, value: Math.max(0, Math.min(1, value / 100)) },
						),
					},
				};
			return lane;
		}),
	};
}

function navigationSlots(options: EncoderSlotOptions) {
	const {
		definition,
		items,
		keyframeItems,
		keyframeIndex,
		laneIndex,
		activeLane,
	} = options;
	return [
		{
			id: "timecode-lane-navigation",
			label: "Lane",
			display: activeLane?.name ?? "—",
			value: laneIndex,
			minimum: 0,
			maximum: Math.max(0, definition.lanes.length - 1),
			fineStep: 1,
			coarseStep: 1,
			disabled: !definition.lanes.length,
			set: (requested: number) => {
				const lane = definition.lanes[clampIndex(requested, definition.lanes.length)];
				if (!lane) return;
				options.setSelectedLaneId(lane.id);
				const first = items.find(
					(item) =>
						item.laneId === lane.id &&
						(item.kind === "speed" || item.kind === "volume"),
				);
				options.setSelection(first?.selection ?? null);
			},
		},
		{
			id: "timecode-keyframe-navigation",
			label: "Keyframe selection",
			display: keyframeItems.length
				? `${keyframeIndex + 1} / ${keyframeItems.length}`
				: "—",
			value: keyframeIndex,
			minimum: 0,
			maximum: Math.max(0, keyframeItems.length - 1),
			fineStep: 1,
			coarseStep: 1,
			disabled: !keyframeItems.length,
			set: (requested: number) => {
				const item = keyframeItems[clampIndex(requested, keyframeItems.length)];
				if (!item) return;
				options.setSelection(item.selection);
				options.onScrub(item.frame);
			},
		},
	];
}

function sharedSlots(options: EncoderSlotOptions) {
	const {
		duration,
		fps,
		frame,
		zoom,
		maximumZoom,
		scrollLeft,
		viewportWidth,
		timelineWidth,
	} = options;
	// The further in an operator has zoomed, the finer a turn of the playhead encoder should be.
	// A second of travel per detent is right at full view and hopeless at ten times in, where a
	// second is most of the window. A frame is as fine as the timeline goes, so that is the floor.
	const playheadCoarseStep = Math.max(1, Math.round(fps / Math.max(1, zoom)));
	const scrollRange = Math.max(0, timelineWidth - viewportWidth);
	return [
		{
			id: "timecode-playhead",
			label: "Playhead",
			display: formatFrame(frame, fps),
			value: frame,
			minimum: 0,
			maximum: duration,
			fineStep: 1,
			coarseStep: playheadCoarseStep,
			set: (value: number) =>
				options.onScrub(Math.max(0, Math.min(duration, Math.round(value)))),
		},
		{
			id: "timecode-timeline-zone",
			label: "Timeline zone",
			display: `${Math.round(zoom * 100)}%`,
			value: zoom,
			minimum: 1,
			maximum: maximumZoom,
			fineStep: 0.05,
			coarseStep: 0.25,
			set: (value: number) =>
				options.setZoom(Math.max(1, Math.min(maximumZoom, value))),
		},
		{
			// Zooming in is only half of looking at something: an operator also has to move the
			// window along the timeline, which until now took a mouse.
			id: "timecode-timeline-scroll",
			label: "Timeline scroll",
			display: scrollRange
				? `${Math.round((scrollLeft / scrollRange) * 100)}%`
				: "Whole timeline",
			value: scrollLeft,
			minimum: 0,
			maximum: scrollRange,
			// A detent moves a tenth of what is on screen, so the step follows the zoom without
			// having to know about it.
			fineStep: Math.max(1, Math.round(viewportWidth * 0.1)),
			coarseStep: Math.max(1, Math.round(viewportWidth * 0.5)),
			disabled: scrollRange === 0,
			set: (value: number) =>
				options.setScrollLeft(Math.max(0, Math.min(scrollRange, value))),
		},
	];
}

function markerSlots(
	options: EncoderSlotOptions,
	marker: TimecodeDefinition["markers"][number],
) {
	const { definition, duration, fps, selection } = options;
	return [
		{
			id: "timecode-marker-frame",
			label: "Marker position",
			display: formatFrame(marker.frame, fps),
			value: marker.frame,
			minimum: 0,
			maximum: duration,
			fineStep: 1,
			coarseStep: fps,
			set: (requested: number) => {
				if (selection?.kind !== "marker") return;
				options.onCommit(
					moveTimelineItem(
						definition,
						selection,
						Math.max(0, Math.min(duration, Math.round(requested))),
					),
				);
			},
		},
		{
			id: "timecode-marker-color",
			label: "Marker color",
			display: markerColorOption(marker.color).name,
			value: markerColorIndex(marker.color),
			minimum: 0,
			maximum: TIMECODE_MARKER_COLORS.length - 1,
			fineStep: 1,
			coarseStep: 1,
			set: (requested: number) => {
				const color =
					TIMECODE_MARKER_COLORS[
						wrappedIndex(requested, TIMECODE_MARKER_COLORS.length)
					]?.value;
				if (!color) return;
				options.onCommit({
					...definition,
					markers: definition.markers.map((candidate) =>
						candidate.id === marker.id ? { ...candidate, color } : candidate,
					),
				});
			},
		},
	];
}

/// The four timings of one Cue, in the order an operator reaches for them.
///
/// A Cue is a wait then a fade on the way in, and the same on the way out, so the deck reads
/// left to right as in delay, in fade, out delay, out fade, with the Cue itself chosen beside
/// them.
function cueSlots(context: CueEncoderContext) {
	const index = context.cues.findIndex(
		(cue) => cue.id === context.selectedCueId,
	);
	const selected = index < 0 ? undefined : context.cues[index];
	const cue = selected?.id
		? context.cueList.cues.find((item) => item.id === selected.id)
		: undefined;
	const inFade = cue ? cue.fade_millis || context.timingDefaults.sequenceFadeMillis : 0;
	const write = (patch: Partial<CueCoreTiming>) => {
		if (!cue?.id) return;
		// The writer draws the edit and reports its own failures, so a refused save ends here.
		void context
			.saveCueList(context.cueListId, {
				...context.cueList,
				cues: context.cueList.cues.map((item) =>
					item.id === cue.id ? { ...item, ...patch } : item,
				),
			})
			.catch(() => undefined);
	};
	const timing = (
		id: string,
		label: string,
		millis: number,
		apply: (value: number) => void,
	) => ({
		id,
		label,
		display: cue ? `${(millis / 1_000).toFixed(2)} s` : "—",
		value: millis,
		minimum: 0,
		maximum: MAXIMUM_CUE_TIMING_MILLIS,
		fineStep: 10,
		coarseStep: 250,
		disabled: !cue,
		set: (requested: number) =>
			apply(
				Math.max(0, Math.min(MAXIMUM_CUE_TIMING_MILLIS, Math.round(requested))),
			),
	});
	return [
		timing("timecode-cue-in-delay", "In delay", cue?.delay_millis ?? 0, (value) =>
			write({ delay_millis: value }),
		),
		timing("timecode-cue-in-fade", "In fade", cue?.fade_millis ?? 0, (value) =>
			write({ fade_millis: value }),
		),
		timing(
			"timecode-cue-out-delay",
			"Out delay",
			cue?.out_delay_link === "in_fade"
				? inFade
				: (cue?.out_delay_millis ?? cue?.delay_millis ?? 0),
			(value) => write({ out_delay_millis: value, out_delay_link: undefined }),
		),
		timing(
			"timecode-cue-out-fade",
			"Out fade",
			cue?.out_fade_link === "release"
				? context.timingDefaults.releaseFadeMillis
				: (cue?.out_fade_millis ?? inFade),
			(value) => write({ out_fade_millis: value, out_fade_link: undefined }),
		),
		{
			id: "timecode-cue-selection",
			label: "Selected Cue",
			// The number and the position are usually the same digit, so printing both read as
			// the cue number twice. The label already says which encoder this is.
			display: selected ? selected.number : "—",
			value: Math.max(0, index),
			minimum: 0,
			maximum: Math.max(0, context.cues.length - 1),
			fineStep: 1,
			coarseStep: 1,
			disabled: !context.cues.length,
			set: (requested: number) => {
				const next = context.cues[clampIndex(requested, context.cues.length)];
				context.setSelectedCueId(next?.id ?? null);
			},
		},
	];
}

function keyframeSlots(options: EncoderSlotOptions) {
	const {
		activeKeyframe,
		activeLane,
		definition,
		duration,
		fps,
		selection,
		speedKeyframe,
		volumeKeyframe,
	} = options;
	const selectedValue =
		speedKeyframe?.bpm ?? (volumeKeyframe?.value ?? 0) * 100;
	return [
		{
			id: "timecode-keyframe-frame",
			label: "Keyframe position",
			display: activeKeyframe ? formatFrame(activeKeyframe.frame, fps) : "—",
			value: activeKeyframe?.frame ?? 0,
			minimum: 0,
			maximum: duration,
			fineStep: 1,
			coarseStep: fps,
			disabled: !activeKeyframe,
			set: (requested: number) => {
				if (!selection || !activeKeyframe) return;
				const next = Math.max(0, Math.min(duration, Math.round(requested)));
				options.onCommit(moveTimelineItem(definition, selection, next));
				options.onScrub(next);
			},
		},
		{
			id: "timecode-keyframe-value",
			label: "Keyframe value",
			display: speedKeyframe
				? `${Math.round(selectedValue)} BPM`
				: volumeKeyframe
					? `${Math.round(selectedValue)}%`
					: "—",
			value: selectedValue,
			minimum: speedKeyframe ? SPEED_GROUP_MIN_BPM : 0,
			maximum: speedKeyframe ? SPEED_GROUP_MAX_BPM : 100,
			fineStep: speedKeyframe ? 0.1 : 1,
			coarseStep: speedKeyframe ? 5 : 10,
			disabled: !activeKeyframe,
			set: (value: number) => {
				if (!selection || !activeLane) return;
				options.onCommit(
					laneWithKeyframeValue(definition, activeLane.id, selection, value),
				);
			},
		},
	];
}

/// Publishes the Timecode editor's encoder deck for the current selection.
export function useTimecodeEncoderSlots(options: EncoderSlotOptions) {
	useEffect(() => {
		const shared = sharedSlots(options);
		const cue = options.cueContext;
		publishTimecodeEncoderDeck(options.encoderOwner, {
			timeline: [...navigationSlots(options), ...shared],
			keyframe: [
				...(cue
					? cueSlots(cue)
					: options.activeMarker
						? markerSlots(options, options.activeMarker)
						: keyframeSlots(options)),
				...shared,
			],
			selectionLabel: cue
				? "Selected Cue"
				: options.activeMarker
					? "Selected Marker"
					: "Selected Keyframe",
		});
	});
}
