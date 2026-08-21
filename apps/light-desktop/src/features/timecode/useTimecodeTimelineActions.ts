import {
	type PointerEvent as ReactPointerEvent,
	useEffect,
	useRef,
} from "react";
import type { CueList } from "../../api/types";
import type { TimecodeDefinition } from "../../api/types/timecode";
import {
	automatedCueClipLength,
	type CueClipTimingDefaults,
} from "./cueClipTiming";
import {
	moveTimelineItem,
	resizeTimelineClip,
	snapTimelineFrame,
	type TimecodeEditorSelection,
} from "./editorModel";

interface CueListOption {
	id: string;
	name: string;
	cues: readonly { id?: string; number: string; name: string }[];
	body?: CueList;
}

type CueListClip = Extract<
	TimecodeDefinition["lanes"][number]["content"],
	{ kind: "cue_list" }
>["clips"][number];

export const DEFAULT_SPEED_GROUP_BPM = 120;

const DEFAULT_CUE_CLIP_TIMING: CueClipTimingDefaults = {
	sequenceFadeMillis: 3_000,
	releaseFadeMillis: 3_000,
};

/// Places a new Cuelist clip: a fully automated Cuelist brings its own length,
/// otherwise the last clip left of the playhead is copied, and an empty lane
/// takes everything from the playhead to the end of the timeline.
export function cueClipPlacement({
	clips,
	cueList,
	frame,
	duration,
	timingDefaults = DEFAULT_CUE_CLIP_TIMING,
}: {
	clips: readonly CueListClip[];
	cueList: CueListOption;
	frame: number;
	duration: number;
	timingDefaults?: CueClipTimingDefaults;
}): Omit<CueListClip, "id"> | null {
	const cues = cueList.cues.flatMap((cue) => (cue.id ? [cue.id] : []));
	const start = Math.max(0, Math.min(Math.round(frame), duration - 1));
	const previous = clips
		.filter((clip) => clip.start_frame <= start)
		.sort((left, right) => left.start_frame - right.start_frame)
		.at(-1);
	const startCueId =
		(previous && cues.includes(previous.start_cue_id)
			? previous.start_cue_id
			: cues[0]) ?? "";
	const endCueId =
		(previous && cues.includes(previous.end_cue_id)
			? previous.end_cue_id
			: cues.at(-1)) ?? "";
	if (!startCueId || !endCueId) return null;
	const automated = cueList.body
		? automatedCueClipLength(
				cueList.body,
				startCueId,
				endCueId,
				start,
				timingDefaults,
			)
		: null;
	const copied = previous
		? Math.max(1, previous.end_frame - previous.start_frame)
		: null;
	const length = automated ?? copied ?? Math.max(1, duration - start);
	return {
		start_frame: start,
		end_frame: Math.min(duration, start + Math.max(1, length)),
		start_cue_id: startCueId,
		end_cue_id: endCueId,
		start_behavior: previous?.start_behavior ?? "state",
		end_behavior: previous?.end_behavior ?? "release",
		cue_starts: previous?.cue_starts?.map((placed) => ({ ...placed })) ?? [],
	};
}

interface DragState {
	selection: TimecodeEditorSelection;
	startX: number;
	startY: number;
	startFrame: number;
	clipEdge?: "start" | "end";
	startVolume?: number;
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
				12,
				{
					selection: active.selection,
					movingClip:
						active.selection.kind === "clip" && active.clipEdge === undefined,
				},
			);
			const next =
				active.clipEdge && active.selection.kind === "clip"
					? resizeTimelineClip(
							latest.current,
							active.selection as TimecodeEditorSelection & { kind: "clip" },
							active.clipEdge,
							snapped,
						)
					: moveTimelineItem(latest.current, active.selection, snapped);
			if (
				active.selection.kind === "volume" &&
				active.startVolume !== undefined
			)
				onPreview(
					setVolumeValue(
						next,
						active.selection,
						Math.max(
							0,
							Math.min(
								1,
								active.startVolume - (event.clientY - active.startY) / 160,
							),
						),
					),
				);
			else onPreview(next);
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
		clipEdge?: "start" | "end",
		startVolume?: number,
	) => {
		event.preventDefault();
		setSelection(selection);
		onBeginGesture();
		drag.current = {
			selection,
			startX: event.clientX,
			startY: event.clientY,
			startFrame,
			clipEdge,
			startVolume,
		};
		event.currentTarget.setPointerCapture?.(event.pointerId);
	};
}

function setVolumeValue(
	definition: TimecodeDefinition,
	selection: TimecodeEditorSelection,
	value: number,
): TimecodeDefinition {
	if (selection.kind !== "volume") return definition;
	return {
		...definition,
		lanes: definition.lanes.map((lane) =>
			lane.id !== selection.laneId || lane.content.kind !== "audio_volume"
				? lane
				: {
						...lane,
						content: {
							...lane.content,
							keyframes: lane.content.keyframes.map((keyframe) =>
								keyframe.id === selection.itemId
									? { ...keyframe, value }
									: keyframe,
							),
						},
					},
		),
	};
}

/// One lane per patched Internal Audio Player; a second lane for the same player is refused.
function audioPlayerLane(
	definition: TimecodeDefinition,
	audioPlayers: readonly { fixtureId: string; name: string }[],
	fixtureId: string,
): TimecodeDefinition | null {
	const player = audioPlayers.find(
		(candidate) => candidate.fixtureId === fixtureId,
	);
	if (
		!player ||
		definition.lanes.some(
			(lane) =>
				lane.content.kind === "audio_player" &&
				lane.content.fixture_id === fixtureId,
		)
	)
		return null;
	return addLane(definition, player.name, {
		kind: "audio_player",
		fixture_id: fixtureId,
		clips: [],
	});
}

/// A new Speed Group lane always starts with one keyframe so the group has a value.
function speedLane(
	definition: TimecodeDefinition,
	group: string,
	laneId: string,
	keyframeId: string,
): TimecodeDefinition {
	return addLane(
		definition,
		`Speed Group ${group}`,
		{
			kind: "speed_group",
			group,
			keyframes: [
				{ id: keyframeId, frame: 0, bpm: DEFAULT_SPEED_GROUP_BPM, phase: 0 },
			],
		},
		laneId,
	);
}

export function useTimelineActions({
	definition,
	frame,
	fps,
	duration,
	cueLists,
	speedGroup,
	audioPlayers,
	timingDefaults,
	onCommit,
	setSelection,
	setSelectedLane,
}: {
	definition: TimecodeDefinition;
	frame: number;
	fps: number;
	duration: number;
	cueLists: readonly CueListOption[];
	speedGroup: string;
	audioPlayers: readonly { fixtureId: string; name: string }[];
	timingDefaults?: CueClipTimingDefaults;
	onCommit(value: TimecodeDefinition): void;
	setSelection(value: TimecodeEditorSelection | null): void;
	setSelectedLane(laneId: string): void;
}) {
	const addMarker = () => {
		const id = crypto.randomUUID();
		onCommit(addMarkerToDefinition(definition, id, frame, duration));
		setSelection({ kind: "marker", itemId: id });
	};
	const addKeyframe = (laneId: string, atFrame = frame) => {
		const id = crypto.randomUUID();
		onCommit(addKeyframeToDefinition(definition, laneId, id, atFrame));
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
	const addSpeedLane = (group = speedGroup) => {
		if (
			definition.lanes.some(
				(lane) =>
					lane.content.kind === "speed_group" && lane.content.group === group,
			)
		)
			return;
		const laneId = crypto.randomUUID();
		const keyframeId = crypto.randomUUID();
		onCommit(speedLane(definition, group, laneId, keyframeId));
		setSelection({ kind: "speed", laneId, itemId: keyframeId });
		setSelectedLane(laneId);
	};
	const addCueListLane = (cueListId: string) => {
		const cueList = cueLists.find((candidate) => candidate.id === cueListId);
		if (!cueList) return;
		const laneId = crypto.randomUUID();
		const clipId = crypto.randomUUID();
		const placement = cueClipPlacement({
			clips: [],
			cueList,
			frame,
			duration,
			timingDefaults,
		});
		onCommit(
			addLane(
				definition,
				cueList.name,
				{
					kind: "cue_list",
					cue_list_id: cueList.id,
					clips: placement ? [{ id: clipId, ...placement }] : [],
				},
				laneId,
			),
		);
		setSelection(placement ? { kind: "clip", laneId, itemId: clipId } : null);
		setSelectedLane(laneId);
	};
	const addAudioPlayerLane = (fixtureId: string) => {
		const next = audioPlayerLane(definition, audioPlayers, fixtureId);
		if (next) onCommit(next);
	};
	const addClip = (laneId: string) => {
		const lane = definition.lanes.find((candidate) => candidate.id === laneId);
		if (!lane) return;
		const id = crypto.randomUUID();
		if (lane.content.kind === "audio_player") {
			const start = Math.min(frame, Math.max(0, duration - fps));
			onCommit(
				addAudioPlayerClipToDefinition(definition, laneId, {
					id,
					start,
					end: Math.min(duration, start + fps * 4),
				}),
			);
			setSelection({ kind: "clip", laneId, itemId: id });
			return;
		}
		if (lane.content.kind !== "cue_list") return;
		const content = lane.content;
		const cueList = cueLists.find(
			(candidate) => candidate.id === content.cue_list_id,
		);
		if (!cueList) return;
		const placement = cueClipPlacement({
			clips: content.clips,
			cueList,
			frame,
			duration,
			timingDefaults,
		});
		if (!placement) return;
		onCommit(addClipToDefinition(definition, laneId, { id, ...placement }));
		setSelection({ kind: "clip", laneId, itemId: id });
	};
	return {
		addMarker,
		addKeyframe,
		addAudioLane,
		addSpeedLane,
		addCueListLane,
		addAudioPlayerLane,
		addClip,
	};
}

function addAudioPlayerClipToDefinition(
	definition: TimecodeDefinition,
	laneId: string,
	clip: { id: string; start: number; end: number },
): TimecodeDefinition {
	return {
		...definition,
		lanes: definition.lanes.map((lane) =>
			lane.id !== laneId || lane.content.kind !== "audio_player"
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
									folder: 0,
									file: 0,
									repeat: false,
									volume_keyframes: [
										{
											id: crypto.randomUUID(),
											frame: clip.start,
											value: 1,
											fade_frames: 0,
											curve: "linear" as const,
										},
									],
								},
							],
						},
					},
		),
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
	id: string = crypto.randomUUID(),
): TimecodeDefinition {
	return {
		...definition,
		lanes: [...definition.lanes, { id, name, content }],
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
							{ id, frame, bpm: DEFAULT_SPEED_GROUP_BPM, phase: 0 },
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
	clip: CueListClip,
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
							clips: [...lane.content.clips, clip],
						},
					},
		),
	};
}
