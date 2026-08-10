import {
	type PointerEvent as ReactPointerEvent,
	useEffect,
	useRef,
} from "react";
import type { TimecodeDefinition } from "../../api/types/timecode";
import {
	moveTimelineItem,
	parseMarkerCsv,
	snapTimelineFrame,
	type TimecodeEditorSelection,
} from "./editorModel";

interface CueListOption {
	id: string;
	name: string;
	cues: readonly { id: string; number: number; name: string }[];
}

interface DragState {
	selection: TimecodeEditorSelection;
	startX: number;
	startFrame: number;
}

export function useTimelineDrag({
	definition,
	pixelsPerFrame,
	onPreview,
	onBeginGesture,
	onEndGesture,
	setSelection,
}: {
	definition: TimecodeDefinition;
	pixelsPerFrame: number;
	onPreview(value: TimecodeDefinition): void;
	onBeginGesture(): void;
	onEndGesture(): void;
	setSelection(value: TimecodeEditorSelection): void;
}) {
	const drag = useRef<DragState | null>(null);
	const latest = useRef(definition);
	latest.current = definition;
	useEffect(() => {
		const move = (event: PointerEvent) => {
			const active = drag.current;
			if (!active) return;
			const proposed =
				active.startFrame + (event.clientX - active.startX) / pixelsPerFrame;
			const snapped = snapTimelineFrame(
				proposed,
				latest.current,
				pixelsPerFrame,
			);
			onPreview(moveTimelineItem(latest.current, active.selection, snapped));
		};
		const up = () => {
			if (drag.current) {
				drag.current = null;
				onEndGesture();
			}
		};
		window.addEventListener("pointermove", move);
		window.addEventListener("pointerup", up);
		window.addEventListener("pointercancel", up);
		return () => {
			window.removeEventListener("pointermove", move);
			window.removeEventListener("pointerup", up);
			window.removeEventListener("pointercancel", up);
		};
	}, [onEndGesture, onPreview, pixelsPerFrame]);
	return (
		event: ReactPointerEvent,
		selection: TimecodeEditorSelection,
		startFrame: number,
	) => {
		event.preventDefault();
		setSelection(selection);
		onBeginGesture();
		drag.current = { selection, startX: event.clientX, startFrame };
		event.currentTarget.setPointerCapture?.(event.pointerId);
	};
}

export function useTimelineActions({
	definition,
	frame,
	fps,
	duration,
	cueLists,
	speedGroup,
	cueListId,
	csvSource,
	csvMode,
	onCommit,
	setSelection,
	setCsvError,
	setCsvOpen,
}: {
	definition: TimecodeDefinition;
	frame: number;
	fps: number;
	duration: number;
	cueLists: readonly CueListOption[];
	speedGroup: string;
	cueListId: string;
	csvSource: string;
	csvMode: "append" | "replace";
	onCommit(value: TimecodeDefinition): void;
	setSelection(value: TimecodeEditorSelection | null): void;
	setCsvError(value: string | null): void;
	setCsvOpen(value: boolean): void;
}) {
	const addMarker = () => {
		const id = crypto.randomUUID();
		onCommit(addMarkerToDefinition(definition, id, frame, duration));
		setSelection({ kind: "marker", itemId: id });
	};
	const addKeyframe = (laneId: string) => {
		const id = crypto.randomUUID();
		onCommit(addKeyframeToDefinition(definition, laneId, id, frame));
		const lane = definition.lanes.find((candidate) => candidate.id === laneId);
		if (lane?.content.kind === "speed_group")
			setSelection({ kind: "speed", laneId, itemId: id });
		if (lane?.content.kind === "audio_volume")
			setSelection({ kind: "volume", laneId, itemId: id });
	};
	const addAudioLane = () => {
		if (!definition.lanes.some((lane) => lane.content.kind === "audio_volume"))
			onCommit(
				addLane(definition, "Main audio volume", {
					kind: "audio_volume",
					keyframes: [],
				}),
			);
	};
	const addSpeedLane = () =>
		onCommit(
			addLane(definition, `Speed Group ${speedGroup}`, {
				kind: "speed_group",
				group: speedGroup,
				keyframes: [],
			}),
		);
	const addCueListLane = () => {
		const cueList = cueLists.find((candidate) => candidate.id === cueListId);
		if (cueList)
			onCommit(
				addLane(definition, cueList.name, {
					kind: "cue_list",
					cue_list_id: cueList.id,
					clips: [],
				}),
			);
	};
	const addClip = (laneId: string) => {
		const lane = definition.lanes.find((candidate) => candidate.id === laneId);
		if (!lane || lane.content.kind !== "cue_list") return;
		const content = lane.content;
		const cueList = cueLists.find(
			(candidate) => candidate.id === content.cue_list_id,
		);
		const first = cueList?.cues[0];
		const last = cueList?.cues.at(-1);
		if (!first || !last) return;
		const id = crypto.randomUUID();
		const start = Math.min(frame, Math.max(0, duration - fps));
		onCommit(
			addClipToDefinition(definition, laneId, {
				id,
				start,
				end: Math.min(duration, start + fps * 4),
				firstId: first.id,
				lastId: last.id,
			}),
		);
		setSelection({ kind: "clip", laneId, itemId: id });
	};
	const importCsv = () => {
		try {
			const imported = parseMarkerCsv(csvSource, fps, duration);
			onCommit({
				...definition,
				markers:
					csvMode === "append"
						? [...definition.markers, ...imported]
						: imported,
			});
			setCsvError(null);
			setCsvOpen(false);
		} catch (reason) {
			setCsvError(reason instanceof Error ? reason.message : String(reason));
		}
	};
	return {
		addMarker,
		addKeyframe,
		addAudioLane,
		addSpeedLane,
		addCueListLane,
		addClip,
		importCsv,
	};
}

function addMarkerToDefinition(
	definition: TimecodeDefinition,
	id: string,
	frame: number,
	duration: number,
): TimecodeDefinition {
	return {
		...definition,
		markers: [
			...definition.markers,
			{
				id,
				frame: Math.min(frame, duration),
				name: `Marker ${definition.markers.length + 1}`,
			},
		],
	};
}

function addLane(
	definition: TimecodeDefinition,
	name: string,
	content: TimecodeDefinition["lanes"][number]["content"],
): TimecodeDefinition {
	return {
		...definition,
		lanes: [...definition.lanes, { id: crypto.randomUUID(), name, content }],
	};
}

function addKeyframeToDefinition(
	definition: TimecodeDefinition,
	laneId: string,
	id: string,
	frame: number,
): TimecodeDefinition {
	return {
		...definition,
		lanes: definition.lanes.map((lane) => {
			if (lane.id !== laneId) return lane;
			if (lane.content.kind === "speed_group")
				return {
					...lane,
					content: {
						...lane.content,
						keyframes: [
							...lane.content.keyframes,
							{ id, frame, bpm: 120, phase: 0 },
						],
					},
				};
			if (lane.content.kind === "audio_volume")
				return {
					...lane,
					content: {
						...lane.content,
						keyframes: [
							...lane.content.keyframes,
							{ id, frame, value: 1, fade_frames: 0, curve: "linear" as const },
						],
					},
				};
			return lane;
		}),
	};
}

function addClipToDefinition(
	definition: TimecodeDefinition,
	laneId: string,
	clip: {
		id: string;
		start: number;
		end: number;
		firstId: string;
		lastId: string;
	},
): TimecodeDefinition {
	return {
		...definition,
		lanes: definition.lanes.map((lane) =>
			lane.id !== laneId || lane.content.kind !== "cue_list"
				? lane
				: {
						...lane,
						content: {
							...lane.content,
							clips: [
								...lane.content.clips,
								{
									id: clip.id,
									start_frame: clip.start,
									end_frame: clip.end,
									start_cue_id: clip.firstId,
									end_cue_id: clip.lastId,
									start_behavior: "state" as const,
									end_behavior: "release" as const,
								},
							],
						},
					},
		),
	};
}
