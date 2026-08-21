import { useEffect } from "react";
import type { TimecodeDefinition } from "../../api/types/timecode";
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

interface EncoderSlotOptions {
	definition: TimecodeDefinition;
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
	encoderOwner: symbol;
	setZoom(value: number): void;
	setSelection(value: TimecodeEditorSelection | null): void;
	setSelectedLaneId(laneId: string): void;
	onScrub(frame: number): void;
	onCommit(value: TimecodeDefinition): void;
}

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
	const { duration, fps, frame, zoom, maximumZoom } = options;
	return [
		{
			id: "timecode-playhead",
			label: "Playhead",
			display: formatFrame(frame, fps),
			value: frame,
			minimum: 0,
			maximum: duration,
			fineStep: 1,
			coarseStep: fps,
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
		publishTimecodeEncoderDeck(options.encoderOwner, {
			timeline: [...navigationSlots(options), ...shared],
			keyframe: [
				...(options.activeMarker
					? markerSlots(options, options.activeMarker)
					: keyframeSlots(options)),
				...shared,
			],
			selectionLabel: options.activeMarker
				? "Selected Marker"
				: "Selected Keyframe",
		});
	});
}
