import {
	Button,
	CheckboxField,
	ColorPickerField,
	NumberField,
	SelectField,
	TextField,
} from "@tosklight/ui";
import {
	forwardRef,
	type PointerEvent as ReactPointerEvent,
	type RefObject,
	useEffect,
	useImperativeHandle,
	useLayoutEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import type { TimecodeDefinition } from "../../api/types/timecode";
import {
	sameSelection,
	deleteTimelineItem,
	moveTimelineItem,
	type TimecodeEditorSelection,
	timelineItems,
} from "./editorModel";
import {
	clearTimecodeEncoderDeck,
	publishTimecodeEncoderDeck,
} from "./timecodeEncoderBridge";
import { CueListChooser } from "./TimecodeCueListChooser";
import {
	TIMECODE_SPEED_GROUPS,
	TimecodeSpeedGroupChooser,
} from "./TimecodeSpeedGroupChooser";
import {
	useTimelineActions,
	useTimelineDrag,
} from "./useTimecodeTimelineActions";

interface Props {
	definition: TimecodeDefinition;
	frame: number;
	fps: number;
	cueLists: readonly TimecodeCueListOption[];
	audioPlayers: readonly TimecodeAudioPlayerOption[];
	waveformPeaks?: readonly number[];
	markersLocked?: boolean;
	onScrub(frame: number): void;
	onCommit(definition: TimecodeDefinition): void;
	onPreview(definition: TimecodeDefinition): void;
	onBeginGesture(): void;
	onEndGesture(): void;
}

export interface TimecodeTimelineEditorHandle {
	addMarker(): void;
	chooseSpeedLane(): void;
	chooseCueListLane(): void;
	addAudioPlayerLane(fixtureId: string): void;
}

type TimelineItem = ReturnType<typeof timelineItems>[number];

export const TIMECODE_LANE_HEADER_WIDTH = 160;

export function timelineFrameX(frame: number, pixelsPerFrame: number): number {
	return TIMECODE_LANE_HEADER_WIDTH + frame * pixelsPerFrame;
}

function TimelineCanvas(props: {
	definition: TimecodeDefinition;
	frame: number;
	fps: number;
	cueLists: readonly TimecodeCueListOption[];
	waveformPeaks?: readonly number[];
	markersLocked?: boolean;
	duration: number;
	width: number;
	pixelsPerFrame: number;
	items: readonly TimelineItem[];
	selection: TimecodeEditorSelection | null;
	scrollRef: RefObject<HTMLDivElement | null>;
	onScrub(frame: number): void;
	startDrag(
		event: ReactPointerEvent,
		selection: TimecodeEditorSelection,
		frame: number,
		clipEdge?: "start" | "end",
	): void;
	addKeyframe(laneId: string, frame?: number): void;
	addClip(laneId: string): void;
	onSelectLane(laneId: string): void;
	selectedLaneId: string | null;
}) {
	const scrub = (event: ReactPointerEvent<HTMLDivElement>) => {
		if (event.target !== event.currentTarget) return;
		scrubAt(event.clientX, event.currentTarget);
	};
	const scrubAt = (clientX: number, canvas: HTMLElement) => {
		const bounds = canvas.getBoundingClientRect();
		props.onScrub(
			Math.max(
				0,
				Math.min(
					props.duration,
					Math.round(
						(clientX - bounds.left - TIMECODE_LANE_HEADER_WIDTH) /
							props.pixelsPerFrame,
					),
				),
			),
		);
	};
	const scrubPlayhead = (event: ReactPointerEvent<HTMLButtonElement>) => {
		const canvas = event.currentTarget.parentElement;
		if (canvas) scrubAt(event.clientX, canvas);
	};
	return (
		<section
			ref={props.scrollRef}
			className="timecode-timeline-scroll"
			aria-label="Timecode timeline viewport"
		>
			<div className="timecode-lane-header-column" aria-hidden="true" />
			<div
				className="timecode-timeline-canvas"
				style={{ width: props.width + TIMECODE_LANE_HEADER_WIDTH }}
				data-pixels-per-frame={props.pixelsPerFrame}
				data-time-origin-px={TIMECODE_LANE_HEADER_WIDTH}
				onPointerDown={scrub}
			>
				<Ruler
					duration={props.duration}
					fps={props.fps}
					pixelsPerFrame={props.pixelsPerFrame}
				/>
				{props.definition.lanes.map((lane) => (
					<EditorLane key={lane.id} {...props} lane={lane} />
				))}
				{props.items
					.filter((item) => item.kind === "marker")
					.map((item) => (
						<TimelineItemButton
							key={item.selection.itemId}
							item={item}
							marker
							selection={props.selection}
							fps={props.fps}
							pixelsPerFrame={props.pixelsPerFrame}
							markersLocked={props.markersLocked}
							startDrag={props.startDrag}
						/>
					))}
				<Button
					className={`timecode-editor-playhead ${timelineFrameX(props.frame, props.pixelsPerFrame) > props.width + TIMECODE_LANE_HEADER_WIDTH - 96 ? "near-right" : ""}`}
					aria-label="Drag playhead to seek"
					style={{ left: timelineFrameX(props.frame, props.pixelsPerFrame) }}
					onPointerDown={(event) => {
						event.preventDefault();
						event.currentTarget.setPointerCapture(event.pointerId);
						scrubPlayhead(event);
					}}
					onPointerMove={(event) => {
						if (!event.currentTarget.hasPointerCapture(event.pointerId)) return;
						scrubPlayhead(event);
					}}
				>
					<span>{formatFrame(props.frame, props.fps)}</span>
				</Button>
			</div>
		</section>
	);
}

function EditorLane(
	props: Parameters<typeof TimelineCanvas>[0] & {
		lane: TimecodeDefinition["lanes"][number];
	},
) {
	const { lane } = props;
	const clipLane =
		lane.content.kind === "cue_list" || lane.content.kind === "audio_player";
	const cueList = lane.content.kind === "cue_list" ? lane.content : null;
	return (
		<div
			className={`timecode-editor-lane lane-${lane.content.kind} ${props.selectedLaneId === lane.id ? "selected" : ""}`}
			onPointerDown={(event) => {
				const target = event.target as Element;
				if (
					target.closest(
						".timecode-timeline-item, .timecode-editor-lane-label button",
					)
				)
					return;
				props.onSelectLane(lane.id);
			}}
		>
			{lane.content.kind === "audio_volume" && (
				<div
					className="timecode-audio-lane-content"
					role="img"
					aria-label="Linked audio waveform"
				>
					{props.waveformPeaks?.length ? (
						<Waveform peaks={props.waveformPeaks} />
					) : (
						<span>Waveform loads after the managed audio is saved.</span>
					)}
					<svg
						className="timecode-audio-keyframe-curve"
						viewBox="0 0 100 100"
						preserveAspectRatio="none"
						aria-hidden="true"
					>
						<polyline
							points={lane.content.keyframes
								.map(
									(keyframe) =>
										`${(keyframe.frame / props.duration) * 100},${100 - keyframe.value * 100}`,
								)
								.join(" ")}
						/>
					</svg>
				</div>
			)}
			<div className="timecode-editor-lane-label">
				<Button
					className="timecode-lane-select"
					active={props.selectedLaneId === lane.id}
					onClick={() => props.onSelectLane(lane.id)}
				>
					<strong>{lane.name}</strong>
					<span>{lane.content.kind.replaceAll("_", " ")}</span>
				</Button>
				{clipLane ? (
					<Button
						size="compact"
						disabled={
							cueList
								? !props.cueLists.find(
										(candidate) => candidate.id === cueList.cue_list_id,
									)?.cues.length
								: false
						}
						onClick={() => props.addClip(lane.id)}
					>
						+ clip
					</Button>
				) : null}
			</div>
			{props.items
				.filter((item) => item.laneId === lane.id)
				.map((item) => (
					<TimelineItemButton
						key={item.selection.itemId}
						item={item}
						selection={props.selection}
						fps={props.fps}
						pixelsPerFrame={props.pixelsPerFrame}
						startDrag={props.startDrag}
						startVolume={
							item.kind === "volume" && lane.content.kind === "audio_volume"
								? lane.content.keyframes.find(
										(keyframe) => keyframe.id === item.selection.itemId,
									)?.value
								: undefined
						}
					/>
				))}
		</div>
	);
}

function TimelineItemButton({
	item,
	marker = false,
	selection,
	fps,
	pixelsPerFrame,
	startDrag,
	startVolume,
	markersLocked = false,
}: {
	item: TimelineItem;
	marker?: boolean;
	selection: TimecodeEditorSelection | null;
	fps: number;
	pixelsPerFrame: number;
	startDrag(
		event: ReactPointerEvent,
		selection: TimecodeEditorSelection,
		frame: number,
		clipEdge?: "start" | "end",
		startVolume?: number,
	): void;
	startVolume?: number;
	markersLocked?: boolean;
}) {
	const width = item.endFrame
		? Math.max(44, (item.endFrame - item.frame) * pixelsPerFrame)
		: undefined;
	return (
		<Button
			className={`${marker ? "timecode-timeline-marker" : `timecode-timeline-item item-${item.kind}`} ${sameSelection(selection, item.selection) ? "selected" : ""}`}
			style={{
				left: timelineFrameX(item.frame, pixelsPerFrame),
				...(marker ? { color: item.color } : {}),
				...(marker ? { width: 44, transform: "translateX(-22px)" } : {}),
				...(width ? { width } : {}),
			}}
			onPointerDown={(event) => {
				if (marker && markersLocked) {
					event.preventDefault();
					return;
				}
				startDrag(event, item.selection, item.frame, undefined, startVolume);
			}}
			aria-disabled={marker && markersLocked ? true : undefined}
			title={`${item.label} · ${formatFrame(item.frame, fps)}`}
		>
			{item.kind === "clip" && item.endFrame !== undefined && (
				<>
					<span
						className="timecode-clip-edge start"
						aria-hidden="true"
						onPointerDown={(event) => {
							event.stopPropagation();
							startDrag(event, item.selection, item.frame, "start");
						}}
					/>
					<span
						className="timecode-clip-edge end"
						aria-hidden="true"
						onPointerDown={(event) => {
							event.stopPropagation();
							startDrag(
								event,
								item.selection,
								item.endFrame ?? item.frame,
								"end",
							);
						}}
					/>
				</>
			)}
			{marker ? (
				<>
					<span className="timecode-timeline-marker-line" aria-hidden="true" />
					<span className="timecode-timeline-marker-label">{item.label}</span>
				</>
			) : (
				(item.valueLabel ?? item.label)
			)}
			{!marker && <small>{formatFrame(item.frame, fps)}</small>}
		</Button>
	);
}

export interface TimecodeCueListOption {
	id: string;
	name: string;
	cues: readonly { id: string; number: string; name: string }[];
}

export interface TimecodeAudioPlayerOption {
	fixtureId: string;
	name: string;
}

const TARGET_MAX_PIXELS_PER_FRAME = 17.5;
const FALLBACK_VIEWPORT_WIDTH = 720;
const TIMECODE_EASINGS: Array<{
	value: "linear" | "ease_in" | "ease_out" | "ease_in_out";
	label: string;
}> = [
	{ value: "linear", label: "Linear" },
	{ value: "ease_in", label: "Ease in" },
	{ value: "ease_out", label: "Ease out" },
	{ value: "ease_in_out", label: "Ease in/out" },
];

function clampIndex(value: number, length: number): number {
	return Math.max(0, Math.min(Math.max(0, length - 1), Math.round(value)));
}

function useTimelineViewportWidth(scrollRef: RefObject<HTMLDivElement | null>) {
	const [viewportWidth, setViewportWidth] = useState(FALLBACK_VIEWPORT_WIDTH);
	useLayoutEffect(() => {
		const element = scrollRef.current;
		if (!element) return;
		const measure = () =>
			setViewportWidth(
				element.clientWidth > 0 ? element.clientWidth : FALLBACK_VIEWPORT_WIDTH,
			);
		measure();
		if (typeof ResizeObserver === "undefined") return;
		const observer = new ResizeObserver(measure);
		observer.observe(element);
		return () => observer.disconnect();
	}, [scrollRef]);
	return viewportWidth;
}

export function timelineZoomGeometry(
	durationFrames: number,
	viewportWidth: number,
) {
	const duration = Math.max(1, durationFrames);
	const viewport = Math.max(1, viewportWidth);
	const fitPixelsPerFrame = Math.min(
		TARGET_MAX_PIXELS_PER_FRAME,
		viewport / duration,
	);
	return {
		fitPixelsPerFrame,
		maximumZoom: Math.max(
			1,
			Math.ceil((TARGET_MAX_PIXELS_PER_FRAME / fitPixelsPerFrame) * 4) / 4,
		),
	};
}

export const TimecodeTimelineEditor = forwardRef<
	TimecodeTimelineEditorHandle,
	Props
>(function TimecodeTimelineEditor(
	{
		definition,
		frame,
		fps,
		cueLists,
		audioPlayers,
		waveformPeaks,
		markersLocked = false,
		onScrub,
		onCommit,
		onPreview,
		onBeginGesture,
		onEndGesture,
	},
	ref,
) {
	const [zoom, setZoom] = useState(1);
	const [selection, setSelection] = useState<TimecodeEditorSelection | null>(
		null,
	);
	const [selectedLaneId, setSelectedLaneId] = useState<string | null>(null);
	const [speedGroup, setSpeedGroup] = useState("A");
	const [speedGroupChooserOpen, setSpeedGroupChooserOpen] = useState(false);
	const [cueListChooserOpen, setCueListChooserOpen] = useState(false);
	const [cueListId, setCueListId] = useState(cueLists[0]?.id ?? "");
	const scrollRef = useRef<HTMLDivElement>(null);
	const viewportWidth = useTimelineViewportWidth(scrollRef);
	const duration = Math.max(1, definition.duration_frame ?? fps * 60);
	const timelineViewportWidth = Math.max(
		1,
		viewportWidth - TIMECODE_LANE_HEADER_WIDTH,
	);
	const geometry = timelineZoomGeometry(duration, timelineViewportWidth);
	const maximumZoom = geometry.maximumZoom;
	const pixelsPerFrame = Math.min(
		TARGET_MAX_PIXELS_PER_FRAME,
		geometry.fitPixelsPerFrame * zoom,
	);
	const width = Math.max(timelineViewportWidth, duration * pixelsPerFrame);
	const items = useMemo(() => timelineItems(definition), [definition]);
	const activeLaneId =
		selection && "laneId" in selection ? selection.laneId : selectedLaneId;
	const encoderOwner = useRef(Symbol("timecode-editor")).current;
	const keyframeItems = items.filter(
		(item) =>
			item.laneId === activeLaneId &&
			(item.kind === "speed" || item.kind === "volume"),
	);
	const laneIndex = Math.max(
		0,
		definition.lanes.findIndex((lane) => lane.id === activeLaneId),
	);
	const keyframeIndex = Math.max(
		0,
		keyframeItems.findIndex((item) => sameSelection(item.selection, selection)),
	);
	const activeLane = definition.lanes.find((lane) => lane.id === activeLaneId);
	const activeKeyframe = keyframeItems.find((item) =>
		sameSelection(item.selection, selection),
	);
	const speedKeyframe =
		selection?.kind === "speed" && activeLane?.content.kind === "speed_group"
			? activeLane.content.keyframes.find(
					(keyframe) => keyframe.id === selection.itemId,
				)
			: undefined;
	const volumeKeyframe =
		selection?.kind === "volume" && activeLane?.content.kind === "audio_volume"
			? activeLane.content.keyframes.find(
					(keyframe) => keyframe.id === selection.itemId,
				)
			: undefined;
	useEffect(() => {
		if (selection && "laneId" in selection) setSelectedLaneId(selection.laneId);
	}, [selection]);

	const startDrag = useTimelineDrag({
		definition,
		pixelsPerFrame,
		onPreview,
		onBeginGesture,
		onEndGesture,
		setSelection,
	});

	const {
		addMarker,
		addKeyframe,
		addSpeedLane,
		addCueListLane,
		addAudioPlayerLane,
		addClip,
	} = useTimelineActions({
		definition,
		frame,
		fps,
		duration,
		cueLists,
		speedGroup,
		audioPlayers,
		onCommit,
		setSelection,
	});
	useImperativeHandle(
		ref,
		() => ({
			addMarker,
			chooseSpeedLane: () => setSpeedGroupChooserOpen(true),
			chooseCueListLane: () => setCueListChooserOpen(true),
			addAudioPlayerLane,
		}),
		[addAudioPlayerLane, addMarker],
	);
	useEffect(() => {
		if (zoom > maximumZoom) setZoom(maximumZoom);
	}, [maximumZoom, zoom]);
	useEffect(() => {
		if (!cueLists.some((cueList) => cueList.id === cueListId))
			setCueListId(cueLists[0]?.id ?? "");
	}, [cueListId, cueLists]);
	const availableSpeedGroups = TIMECODE_SPEED_GROUPS.filter(
		(group) =>
			!definition.lanes.some(
				(lane) =>
					lane.content.kind === "speed_group" && lane.content.group === group,
			),
	);
	useEffect(() => {
		if (!availableSpeedGroups.includes(speedGroup as never))
			setSpeedGroup(availableSpeedGroups[0] ?? "");
	}, [availableSpeedGroups, speedGroup]);
	useEffect(() => {
		const selectLane = (requested: number) => {
			const index = clampIndex(requested, definition.lanes.length);
			const lane = definition.lanes[index];
			if (!lane) return;
			setSelectedLaneId(lane.id);
			const first = items.find(
				(item) =>
					item.laneId === lane.id &&
					(item.kind === "speed" || item.kind === "volume"),
			);
			setSelection(first?.selection ?? null);
		};
		const selectKeyframe = (requested: number) => {
			const item = keyframeItems[clampIndex(requested, keyframeItems.length)];
			if (!item) return;
			setSelection(item.selection);
			onScrub(item.frame);
		};
		const setSelectedFrame = (requested: number) => {
			if (!selection || !activeKeyframe) return;
			const next = Math.max(0, Math.min(duration, Math.round(requested)));
			onCommit(moveTimelineItem(definition, selection, next));
			onScrub(next);
		};
		const updateSelected = (
			value: number,
			field: "value" | "aux" | "easing",
		) => {
			if (!selection || !activeLane) return;
			onCommit({
				...definition,
				lanes: definition.lanes.map((lane) => {
					if (lane.id !== activeLane.id) return lane;
					if (
						selection.kind === "speed" &&
						lane.content.kind === "speed_group"
					)
						return {
							...lane,
							content: {
								...lane.content,
								keyframes: lane.content.keyframes.map((keyframe) =>
									keyframe.id !== selection.itemId
										? keyframe
										: field === "value"
											? { ...keyframe, bpm: Math.max(1, Math.min(999, value)) }
											: field === "aux"
												? { ...keyframe, phase: value }
												: keyframe,
								),
							},
						};
					if (
						selection.kind === "volume" &&
						lane.content.kind === "audio_volume"
					)
						return {
							...lane,
							content: {
								...lane.content,
								keyframes: lane.content.keyframes.map((keyframe) => {
									if (keyframe.id !== selection.itemId) return keyframe;
									if (field === "value")
										return {
											...keyframe,
											value: Math.max(0, Math.min(1, value / 100)),
										};
									if (field === "aux")
										return {
											...keyframe,
											fade_frames: Math.max(0, Math.round(value)),
										};
									return {
										...keyframe,
										curve: TIMECODE_EASINGS[clampIndex(value, TIMECODE_EASINGS.length)]
											?.value ?? "linear",
									};
								}),
							},
						};
					return lane;
				}),
			});
		};
		const selectedValue = speedKeyframe?.bpm ?? (volumeKeyframe?.value ?? 0) * 100;
		const selectedAux = speedKeyframe?.phase ?? volumeKeyframe?.fade_frames ?? 0;
		const easingIndex = Math.max(
			0,
			TIMECODE_EASINGS.findIndex(
				(easing) => easing.value === volumeKeyframe?.curve,
			),
		);
		publishTimecodeEncoderDeck(encoderOwner, {
			timeline: [
				{
					id: "timecode-zoom",
					label: "Timeline zoom",
					display: `${Math.round(zoom * 100)}%`,
					value: zoom,
					minimum: 1,
					maximum: maximumZoom,
					fineStep: 0.05,
					coarseStep: 0.25,
					set: (value) => setZoom(Math.max(1, Math.min(maximumZoom, value))),
				},
				{
					id: "timecode-playhead",
					label: "Playhead",
					display: formatFrame(frame, fps),
					value: frame,
					minimum: 0,
					maximum: duration,
					fineStep: 1,
					coarseStep: fps,
					set: (value) => onScrub(Math.max(0, Math.min(duration, Math.round(value)))),
				},
				{
					id: "timecode-keyframe-navigation",
					label: "Keyframe",
					display: keyframeItems.length
						? `${keyframeIndex + 1} / ${keyframeItems.length}`
						: "—",
					value: keyframeIndex,
					minimum: 0,
					maximum: Math.max(0, keyframeItems.length - 1),
					fineStep: 1,
					coarseStep: 1,
					disabled: !keyframeItems.length,
					set: selectKeyframe,
				},
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
					set: selectLane,
				},
			],
			keyframe: [
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
					set: setSelectedFrame,
				},
				{
					id: "timecode-keyframe-value",
					label: speedKeyframe ? "BPM" : "Volume",
					display: speedKeyframe
						? `${Math.round(selectedValue)} BPM`
						: volumeKeyframe
							? `${Math.round(selectedValue)}%`
							: "—",
					value: selectedValue,
					minimum: speedKeyframe ? 1 : 0,
					maximum: speedKeyframe ? 999 : 100,
					fineStep: 1,
					coarseStep: speedKeyframe ? 5 : 10,
					disabled: !activeKeyframe,
					set: (value) => updateSelected(value, "value"),
				},
				{
					id: "timecode-keyframe-aux",
					label: speedKeyframe ? "Phase" : "Fade frames",
					display: activeKeyframe ? String(Math.round(selectedAux)) : "—",
					value: selectedAux,
					minimum: 0,
					maximum: speedKeyframe ? 360 : duration,
					fineStep: 1,
					coarseStep: speedKeyframe ? 10 : fps,
					disabled: !activeKeyframe,
					set: (value) => updateSelected(value, "aux"),
				},
				{
					id: "timecode-keyframe-easing",
					label: "Easing",
					display: volumeKeyframe
						? (TIMECODE_EASINGS[easingIndex]?.label ?? "Linear")
						: "—",
					value: easingIndex,
					minimum: 0,
					maximum: TIMECODE_EASINGS.length - 1,
					fineStep: 1,
					coarseStep: 1,
					disabled: !volumeKeyframe,
					set: (value) => updateSelected(value, "easing"),
				},
			],
		});
	});
	useEffect(
		() => () => clearTimecodeEncoderDeck(encoderOwner),
		[encoderOwner],
	);

	return (
		<section
			className="timecode-timeline-editor"
			aria-label="Timecode timeline editor"
		>
			<TimelineCanvas
				{...{
					definition,
					frame,
					fps,
					cueLists,
					waveformPeaks,
					markersLocked,
					duration,
					width,
					pixelsPerFrame,
					items,
					selection,
					scrollRef,
					onScrub,
					startDrag,
					addKeyframe,
					addClip,
					onSelectLane: (laneId: string) => {
						setSelectedLaneId(laneId);
						const first = items.find((item) => item.laneId === laneId);
						setSelection(first?.selection ?? null);
					},
					selectedLaneId: activeLaneId,
				}}
			/>
			<KeyframeActionStrip
				definition={definition}
				selection={selection}
				laneId={activeLaneId}
				frame={frame}
				items={items}
				onSelection={(item) => setSelection(item.selection)}
				onInsert={() => {
					if (activeLaneId) addKeyframe(activeLaneId, frame);
				}}
				onCommit={onCommit}
			/>
			{cueListChooserOpen && (
				<CueListChooser
					cueLists={cueLists}
					value={cueListId}
					onChange={setCueListId}
					onClose={() => setCueListChooserOpen(false)}
					onAdd={() => {
						if (cueListId) addCueListLane(cueListId);
						setCueListChooserOpen(false);
					}}
				/>
			)}
			{speedGroupChooserOpen && (
				<TimecodeSpeedGroupChooser
					available={availableSpeedGroups}
					value={speedGroup}
					onChange={setSpeedGroup}
					onClose={() => setSpeedGroupChooserOpen(false)}
					onAdd={() => {
						if (speedGroup) addSpeedLane(speedGroup);
						setSpeedGroupChooserOpen(false);
					}}
				/>
			)}
		</section>
	);
});

function KeyframeActionStrip({
	definition,
	selection,
	laneId,
	frame,
	items,
	onSelection,
	onInsert,
	onCommit,
}: {
	definition: TimecodeDefinition;
	selection: TimecodeEditorSelection | null;
	laneId: string | null;
	frame: number;
	items: readonly TimelineItem[];
	onSelection(item: TimelineItem): void;
	onInsert(): void;
	onCommit(definition: TimecodeDefinition): void;
}) {
	const lane = definition.lanes.find((candidate) => candidate.id === laneId);
	const laneItems = items.filter((item) => item.laneId === laneId);
	const selectedIndex = laneItems.findIndex((item) =>
		sameSelection(item.selection, selection),
	);
	const selectedVolume =
		selection?.kind === "volume" && lane?.content.kind === "audio_volume"
			? lane.content.keyframes.find(
					(keyframe) => keyframe.id === selection.itemId,
				)
			: undefined;
	const move = (delta: number) => {
		if (!laneItems.length) return;
		const index =
			selectedIndex < 0
				? delta < 0
					? laneItems.length - 1
					: 0
				: Math.max(0, Math.min(laneItems.length - 1, selectedIndex + delta));
		const item = laneItems[index];
		if (item) onSelection(item);
	};
	const updateEasing = (curve: string) => {
		if (!selectedVolume || selection?.kind !== "volume" || !lane) return;
		onCommit({
			...definition,
			lanes: definition.lanes.map((candidate) =>
				candidate.id !== lane.id || candidate.content.kind !== "audio_volume"
					? candidate
					: {
							...candidate,
							content: {
								...candidate.content,
								keyframes: candidate.content.keyframes.map((keyframe) =>
									keyframe.id === selectedVolume.id
										? {
												...keyframe,
												curve: curve as typeof keyframe.curve,
											}
										: keyframe,
								),
							},
						},
			),
		});
	};
	const canInsert =
		lane?.content.kind === "audio_volume" ||
		lane?.content.kind === "speed_group";
	const canDelete =
		selection?.kind === "volume" || selection?.kind === "speed";
	return (
		<div
			className="timecode-keyframe-actions"
			aria-label="Selected lane and keyframe actions"
		>
			<strong>{lane?.name ?? "Select a lane"}</strong>
			<Button disabled={!laneItems.length} onClick={() => move(-1)}>
				Previous keyframe
			</Button>
			<Button disabled={!laneItems.length} onClick={() => move(1)}>
				Next keyframe
			</Button>
			<Button disabled={!canInsert} onClick={onInsert}>
				Insert keyframe at {formatFrame(frame, 44)}
			</Button>
			<SelectField
				label="Easing"
				value={selectedVolume?.curve ?? "linear"}
				disabled={!selectedVolume}
				onChange={updateEasing}
				options={TIMECODE_EASINGS}
			/>
			<Button
				disabled={!canDelete}
				onClick={() => {
					if (canDelete && selection)
						onCommit(deleteTimelineItem(definition, selection));
				}}
			>
				Delete selected keyframe
			</Button>
		</div>
	);
}

interface SelectionInspectorProps {
	definition: TimecodeDefinition;
	selection: TimecodeEditorSelection | null;
	selectedLabel?: string;
	cueLists: readonly TimecodeCueListOption[];
	fps: number;
	onCommit(definition: TimecodeDefinition): void;
}

function SelectionInspector({
	definition,
	selection,
	selectedLabel,
	cueLists,
	fps,
	onCommit,
}: SelectionInspectorProps) {
	if (!selection)
		return (
			<div className="timecode-selection-inspector">
				<span>
					Select a clip, keyframe, or marker to inspect, copy, move, or delete
					it.
				</span>
			</div>
		);
	if (selection.kind === "marker")
		return (
			<SelectedMarkerInspector
				{...{ definition, selection, selectedLabel, fps, onCommit }}
			/>
		);
	const lane = definition.lanes.find(
		(candidate) => candidate.id === selection.laneId,
	);
	if (!lane) return null;
	if (selection.kind === "speed" && lane.content.kind === "speed_group") {
		const content = lane.content;
		const keyframe = content.keyframes.find(
			(candidate) => candidate.id === selection.itemId,
		);
		if (!keyframe) return null;
		return (
			<SpeedInspector
				label={selectedLabel}
				frame={keyframe.frame}
				fps={fps}
				bpm={keyframe.bpm}
				phase={keyframe.phase}
				onBpm={(bpm) =>
					onCommit(
						updateLane(definition, lane.id, {
							...content,
							keyframes: content.keyframes.map((candidate) =>
								candidate.id === keyframe.id
									? { ...candidate, bpm }
									: candidate,
							),
						}),
					)
				}
				onPhase={(phase) =>
					onCommit(
						updateLane(definition, lane.id, {
							...content,
							keyframes: content.keyframes.map((candidate) =>
								candidate.id === keyframe.id
									? { ...candidate, phase }
									: candidate,
							),
						}),
					)
				}
			/>
		);
	}
	if (selection.kind === "volume" && lane.content.kind === "audio_volume") {
		const content = lane.content;
		const keyframe = content.keyframes.find(
			(candidate) => candidate.id === selection.itemId,
		);
		if (!keyframe) return null;
		const update = (patch: Partial<typeof keyframe>) =>
			onCommit(
				updateLane(definition, lane.id, {
					...content,
					keyframes: content.keyframes.map((candidate) =>
						candidate.id === keyframe.id
							? { ...candidate, ...patch }
							: candidate,
					),
				}),
			);
		return (
			<VolumeInspector
				label={selectedLabel}
				keyframe={keyframe}
				fps={fps}
				update={update}
			/>
		);
	}
	if (selection.kind === "clip" && lane.content.kind === "cue_list") {
		const content = lane.content;
		const clip = content.clips.find(
			(candidate) => candidate.id === selection.itemId,
		);
		if (!clip) return null;
		const cues =
			cueLists.find((candidate) => candidate.id === content.cue_list_id)
				?.cues ?? [];
		const update = (patch: Partial<typeof clip>) =>
			onCommit(
				updateLane(definition, lane.id, {
					...content,
					clips: content.clips.map((candidate) =>
						candidate.id === clip.id ? { ...candidate, ...patch } : candidate,
					),
				}),
			);
		return (
			<ClipInspector
				label={selectedLabel}
				clip={clip}
				cues={cues}
				duration={definition.duration_frame}
				update={update}
			/>
		);
	}
	if (selection.kind === "clip" && lane.content.kind === "audio_player") {
		const content = lane.content;
		const clip = content.clips.find(
			(candidate) => candidate.id === selection.itemId,
		);
		if (!clip) return null;
		const update = (patch: Partial<typeof clip>) =>
			onCommit(
				updateLane(definition, lane.id, {
					...content,
					clips: content.clips.map((candidate) =>
						candidate.id === clip.id ? { ...candidate, ...patch } : candidate,
					),
				}),
			);
		return (
			<AudioPlayerClipInspector
				label={selectedLabel}
				clip={clip}
				duration={definition.duration_frame}
				update={update}
			/>
		);
	}
	return null;
}

function SelectedMarkerInspector({
	definition,
	selection,
	selectedLabel,
	fps,
	onCommit,
}: Omit<SelectionInspectorProps, "cueLists"> & {
	selection: Extract<TimecodeEditorSelection, { kind: "marker" }>;
}) {
	const marker = definition.markers.find(
		(candidate) => candidate.id === selection.itemId,
	);
	if (!marker) return null;
	return (
		<MarkerInspector
			{...{ definition, marker, selectedLabel, fps, onCommit }}
		/>
	);
}

type AudioPlayerClip = Extract<
	TimecodeDefinition["lanes"][number]["content"],
	{ kind: "audio_player" }
>["clips"][number];

function AudioPlayerClipInspector({
	label,
	clip,
	duration,
	update,
}: {
	label?: string;
	clip: AudioPlayerClip;
	duration?: number | null;
	update(patch: Partial<AudioPlayerClip>): void;
}) {
	const updateVolume = (
		id: string,
		patch: Partial<AudioPlayerClip["volume_keyframes"][number]>,
	) =>
		update({
			volume_keyframes: clip.volume_keyframes.map((keyframe) =>
				keyframe.id === id ? { ...keyframe, ...patch } : keyframe,
			),
		});
	const addVolumePoint = () => {
		const previous = clip.volume_keyframes.at(-1);
		const frame = Math.min(
			clip.end_frame - 1,
			Math.max(
				clip.start_frame,
				previous
					? previous.frame + 1
					: Math.round((clip.start_frame + clip.end_frame) / 2),
			),
		);
		update({
			volume_keyframes: [
				...clip.volume_keyframes,
				{
					id: crypto.randomUUID(),
					frame,
					value: previous?.value ?? 1,
					fade_frames: 0,
					curve: "linear" as const,
				},
			].sort((left, right) => left.frame - right.frame),
		});
	};
	return (
		<div className="timecode-selection-inspector">
			<strong>{label}</strong>
			<InspectorNumber
				label="Start frame"
				value={clip.start_frame}
				min={0}
				max={clip.end_frame - 1}
				onValue={(start_frame) => {
					const offset = start_frame - clip.start_frame;
					update({
						start_frame,
						volume_keyframes: clip.volume_keyframes.map((keyframe) => ({
							...keyframe,
							frame: keyframe.frame + offset,
						})),
					});
				}}
			/>
			<InspectorNumber
				label="End frame"
				value={clip.end_frame}
				min={clip.start_frame + 1}
				max={duration ?? undefined}
				onValue={(end_frame) => update({ end_frame })}
			/>
			<InspectorNumber
				label="Audio Folder"
				value={clip.folder}
				min={0}
				max={255}
				onValue={(folder) => update({ folder })}
			/>
			<InspectorNumber
				label="Audio File"
				value={clip.file}
				min={0}
				max={255}
				onValue={(file) => update({ file })}
			/>
			<CheckboxField
				label="Repeat"
				stateLabel="Repeat clip"
				checked={clip.repeat}
				onChange={(event) => update({ repeat: event.currentTarget.checked })}
			/>
			{clip.volume_keyframes.map((keyframe, index) => (
				<div className="timecode-audio-player-volume-point" key={keyframe.id}>
					<strong>Volume point {index + 1}</strong>
					<InspectorNumber
						label="Volume frame"
						value={keyframe.frame}
						min={clip.start_frame}
						max={clip.end_frame - 1}
						onValue={(frame) => updateVolume(keyframe.id, { frame })}
					/>
					<InspectorNumber
						label="Volume %"
						value={Math.round(keyframe.value * 100)}
						min={0}
						max={100}
						onValue={(value) =>
							updateVolume(keyframe.id, { value: value / 100 })
						}
					/>
					<InspectorNumber
						label="Fade frames"
						value={keyframe.fade_frames}
						min={0}
						onValue={(fade_frames) =>
							updateVolume(keyframe.id, { fade_frames })
						}
					/>
					<SelectField
						label="Volume curve"
						value={keyframe.curve}
						onChange={(curve) => updateVolume(keyframe.id, { curve })}
						options={[
							{ value: "linear", label: "Linear" },
							{ value: "ease_in", label: "Ease in" },
							{ value: "ease_out", label: "Ease out" },
							{ value: "ease_in_out", label: "Ease in/out" },
						]}
					/>
					<Button
						size="compact"
						disabled={clip.volume_keyframes.length === 1}
						onClick={() =>
							update({
								volume_keyframes: clip.volume_keyframes.filter(
									(candidate) => candidate.id !== keyframe.id,
								),
							})
						}
					>
						Remove volume point
					</Button>
				</div>
			))}
			<Button size="compact" onClick={addVolumePoint}>
				Add volume point
			</Button>
		</div>
	);
}

function MarkerInspector({
	definition,
	marker,
	selectedLabel,
	fps,
	onCommit,
}: {
	definition: TimecodeDefinition;
	marker: TimecodeDefinition["markers"][number];
	selectedLabel?: string;
	fps: number;
	onCommit(value: TimecodeDefinition): void;
}) {
	const update = (patch: Partial<typeof marker>) =>
		onCommit({
			...definition,
			markers: definition.markers.map((candidate) =>
				candidate.id === marker.id ? { ...candidate, ...patch } : candidate,
			),
		});
	return (
		<div className="timecode-selection-inspector">
			<strong>{selectedLabel}</strong>
			<TextField
				id={`timecode-marker-name-${marker.id}`}
				label="Name"
				value={marker.name}
				onChange={(event) => update({ name: event.currentTarget.value })}
			/>
			<ColorPickerField
				label="Color"
				value={marker.color ?? "#a67cff"}
				onChange={(color) => update({ color })}
			/>
			<span>Trigger {formatFrame(marker.frame, fps)}</span>
		</div>
	);
}

function SpeedInspector({
	label,
	frame,
	fps,
	bpm,
	phase,
	onBpm,
	onPhase,
}: {
	label?: string;
	frame: number;
	fps: number;
	bpm: number;
	phase: number;
	onBpm(value: number): void;
	onPhase(value: number): void;
}) {
	return (
		<div className="timecode-selection-inspector">
			<strong>{label}</strong>
			<InspectorNumber
				label="BPM"
				value={bpm}
				min={1}
				max={999}
				onValue={onBpm}
			/>
			<InspectorNumber
				label="Phase"
				value={phase}
				min={0}
				max={1}
				step={0.01}
				onValue={onPhase}
			/>
			<span>Trigger {formatFrame(frame, fps)}</span>
		</div>
	);
}

type VolumeKeyframe = Extract<
	TimecodeDefinition["lanes"][number]["content"],
	{ kind: "audio_volume" }
>["keyframes"][number];
function VolumeInspector({
	label,
	keyframe,
	fps,
	update,
}: {
	label?: string;
	keyframe: VolumeKeyframe;
	fps: number;
	update(patch: Partial<VolumeKeyframe>): void;
}) {
	return (
		<div className="timecode-selection-inspector">
			<strong>{label}</strong>
			<InspectorNumber
				label="Volume %"
				value={Math.round(keyframe.value * 100)}
				min={0}
				max={100}
				onValue={(value) => update({ value: value / 100 })}
			/>
			<InspectorNumber
				label="Fade frames"
				value={keyframe.fade_frames}
				min={0}
				onValue={(fade_frames) => update({ fade_frames })}
			/>
			<SelectField
				label="Curve"
				value={keyframe.curve}
				onChange={(curve) => update({ curve })}
				options={[
					{ value: "linear", label: "Linear" },
					{ value: "ease_in", label: "Ease in" },
					{ value: "ease_out", label: "Ease out" },
					{ value: "ease_in_out", label: "Ease in/out" },
				]}
			/>
			<span>Trigger {formatFrame(keyframe.frame, fps)}</span>
		</div>
	);
}

type CueClip = Extract<
	TimecodeDefinition["lanes"][number]["content"],
	{ kind: "cue_list" }
>["clips"][number];
function ClipInspector({
	label,
	clip,
	cues,
	duration,
	update,
}: {
	label?: string;
	clip: CueClip;
	cues: readonly { id: string; number: string; name: string }[];
	duration?: number | null;
	update(patch: Partial<CueClip>): void;
}) {
	return (
		<div className="timecode-selection-inspector">
			<strong>{label}</strong>
			<InspectorNumber
				label="Start frame"
				value={clip.start_frame}
				min={0}
				max={clip.end_frame - 1}
				onValue={(start_frame) => update({ start_frame })}
			/>
			<InspectorNumber
				label="End frame"
				value={clip.end_frame}
				min={clip.start_frame + 1}
				max={duration ?? undefined}
				onValue={(end_frame) => update({ end_frame })}
			/>
			<CueSelect
				label="Start Cue"
				value={clip.start_cue_id}
				cues={cues}
				onValue={(start_cue_id) => update({ start_cue_id })}
			/>
			<CueSelect
				label="End Cue"
				value={clip.end_cue_id}
				cues={cues}
				onValue={(end_cue_id) => update({ end_cue_id })}
			/>
			<SelectField
				label="Start behavior"
				value={clip.start_behavior}
				onChange={(start_behavior) => update({ start_behavior })}
				options={[
					{ value: "state", label: "State Start" },
					{ value: "cue", label: "Cue Start" },
				]}
			/>
			<SelectField
				label="End behavior"
				value={clip.end_behavior}
				onChange={(end_behavior) => update({ end_behavior })}
				options={[
					{ value: "release", label: "Release" },
					{ value: "hold", label: "Hold" },
				]}
			/>
		</div>
	);
}

function InspectorNumber({
	label,
	value,
	min,
	max,
	step,
	onValue,
}: {
	label: string;
	value: number;
	min?: number;
	max?: number;
	step?: number;
	onValue(value: number): void;
}) {
	return (
		<NumberField
			label={label}
			value={value}
			min={min}
			max={max}
			step={step}
			onChange={(event) => onValue(Number(event.currentTarget.value))}
		/>
	);
}

function CueSelect({
	label,
	value,
	cues,
	onValue,
}: {
	label: string;
	value: string;
	cues: readonly { id: string; number: string; name: string }[];
	onValue(value: string): void;
}) {
	return (
		<SelectField
			label={label}
			value={value}
			onChange={onValue}
			options={cues.map((cue) => ({
				value: cue.id,
				label: `${cue.number} · ${cue.name}`,
			}))}
		/>
	);
}

function updateLane(
	definition: TimecodeDefinition,
	laneId: string,
	content: TimecodeDefinition["lanes"][number]["content"],
): TimecodeDefinition {
	return {
		...definition,
		lanes: definition.lanes.map((lane) =>
			lane.id === laneId ? { ...lane, content } : lane,
		),
	};
}

function Ruler({
	duration,
	fps,
	pixelsPerFrame,
}: {
	duration: number;
	fps: number;
	pixelsPerFrame: number;
}) {
	const step = pixelsPerFrame * fps >= 52 ? fps : fps * 5;
	const ticks = Array.from(
		{ length: Math.floor(duration / step) + 1 },
		(_, index) => index * step,
	);
	return (
		<>
			<div className="timecode-ruler-stripes" aria-hidden="true">
				{ticks.map((tick) => (
					<i key={tick} style={{ left: tick * pixelsPerFrame }} />
				))}
			</div>
			<div className="timecode-ruler">
				{ticks.map((tick) => (
					<span
						key={tick}
						className={
							tick === duration ? "timecode-ruler-final-tick" : undefined
						}
						style={{ left: tick * pixelsPerFrame }}
					>
						{formatFrame(tick, fps)}
					</span>
				))}
			</div>
		</>
	);
}

function Waveform({ peaks }: { peaks: readonly number[] }) {
	return (
		<svg
			viewBox={`0 0 ${peaks.length} 48`}
			preserveAspectRatio="none"
			aria-hidden="true"
		>
			{peaks.map((peak, index) => (
				<line
					key={`${index}-${peak}`}
					x1={index}
					x2={index}
					y1={24 - peak * 22}
					y2={24 + peak * 22}
				/>
			))}
		</svg>
	);
}

function formatFrame(frame: number, fps: number): string {
	const whole = Math.max(0, Math.round(frame));
	const seconds = Math.floor(whole / fps);
	return `${String(Math.floor(seconds / 3600)).padStart(2, "0")}:${String(Math.floor(seconds / 60) % 60).padStart(2, "0")}:${String(seconds % 60).padStart(2, "0")}.${String(whole % fps).padStart(2, "0")}`;
}
