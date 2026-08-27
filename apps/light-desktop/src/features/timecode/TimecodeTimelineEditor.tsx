import {
	Button,
	CheckboxField,
	Input,
	InputModal,
	NumberField,
	SelectField,
	TextField,
} from "@tosklight/ui";
import {
	type CSSProperties,
	forwardRef,
	type PointerEvent as ReactPointerEvent,
	type RefObject,
	useEffect,
	useId,
	useImperativeHandle,
	useLayoutEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import type { CueList } from "../../api/types";
import type {
	TimecodeCueListClip,
	TimecodeCueListClipStatus,
	TimecodeDefinition,
} from "../../api/types/timecode";
import {
	SPEED_GROUP_MAX_BPM,
	SPEED_GROUP_MIN_BPM,
} from "../speedGroupRuntime/contracts";
import {
	type CueClipTimingDefaults,
	type CueClipTimingRow,
	type CueFadeEdge,
	type CueFadeKind,
	cueClipTimingRows,
	cueWithDraggedFade,
	TIMECODE_FPS,
} from "./cueClipTiming";
import {
	deleteTimelineItem,
	moveTimelineItem,
	reorderTimelineLane,
	sameSelection,
	type TimecodeEditorSelection,
	timelineItems,
	withClipFade,
	withPlacedCueStart,
} from "./editorModel";
import { type ClipFadeKind, CueListClipBody } from "./TimecodeClipBody";
import { LaneLabel, Waveform } from "./TimecodeLaneParts";
import {
	CueListClipContents,
	CueListClipStatus,
} from "./TimecodeCueClipContents";
import {
	KeyframeActionStrip,
	scaleClipCueTimings,
	useCueEncoderContext,
	useSelectedCue,
} from "./TimecodeKeyframeActions";
import { CueListChooser } from "./TimecodeCueListChooser";
import { MarkerActionStrip } from "./TimecodeMarkerActions";
import {
	formatFrame,
	markerColorIndex,
	MarkerColorButton,
	markerColorOption,
	parseTimelineFrame,
	AudioLaneFileName,
	TIMECODE_LANE_HEADER_WIDTH,
	timelineScroller,
	type OverviewResize,
	TIMECODE_MARKER_COLORS,
	type TimecodeAudioPlayerOption,
	type TimecodeCueListOption,
	type TimelineItem,
	timelineFrameX,
	wrappedIndex,
} from "./timecodeEditorShared";
import {
	TIMECODE_SPEED_GROUPS,
	TimecodeSpeedGroupChooser,
} from "./TimecodeSpeedGroupChooser";
import { clearTimecodeEncoderDeck } from "./timecodeEncoderBridge";
import {
	laneWithKeyframeValue,
	useTimecodeEncoderSlots,
} from "./timecodeEncoderSlots";
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
	clipStatuses?: readonly TimecodeCueListClipStatus[];
	timingDefaults?: CueClipTimingDefaults;
	onScrub(frame: number): void;
	onCommit(definition: TimecodeDefinition): void;
	onPreview(definition: TimecodeDefinition): void;
	onBeginGesture(): void;
	onEndGesture(): void;
	onSaveCueList?(cueListId: string, body: CueList): Promise<CueList>;
	onCueTimingError?(message: string): void;
}

export {
	TIMECODE_LANE_HEADER_WIDTH,
	timelineFrameX,
	type TimecodeAudioPlayerOption,
	type TimecodeCueListOption,
};

export interface TimecodeTimelineEditorHandle {
	addMarker(): void;
	chooseSpeedLane(): void;
	chooseCueListLane(): void;
	addAudioPlayerLane(fixtureId: string): void;
}


function TimelineCanvas(props: {
	definition: TimecodeDefinition;
	frame: number;
	fps: number;
	waveformPeaks?: readonly number[];
	markersLocked?: boolean;
	clipStatuses: readonly TimecodeCueListClipStatus[];
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
		frame: number, clipEdge?: "start" | "end",
	): void;
	startLaneDrag(event: ReactPointerEvent, laneId: string): void;
	consumeLaneDragClick(laneId: string): boolean;
	onSelectItem(selection: TimecodeEditorSelection): void;
	onSelectLane(laneId: string): void;
	onScroll(scrollLeft: number): void;
	selectedLaneId: string | null;
	viewportId: string;
	cueLists: readonly TimecodeCueListOption[];
	timingDefaults: CueClipTimingDefaults;
	onSaveCueList?(cueListId: string, body: CueList): Promise<CueList>;
	onPlaceCueStart(
		laneId: string,
		clipId: string,
		cueId: string,
		offsetFrame: number,
	): void;
	onSetClipFade(
		laneId: string,
		clipId: string,
		kind: ClipFadeKind,
		frames: number,
	): void;
	onCueTimingError?(message: string): void;
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
			id={props.viewportId}
			ref={props.scrollRef}
			className="timecode-timeline-scroll"
			aria-label="Timecode timeline viewport"
			onScroll={(event) => props.onScroll(event.currentTarget.scrollLeft)}
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
							onSelect={props.onSelectItem}
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

function TimelineOverview({
	definition,
	items,
	duration,
	scrollLeft,
	viewportWidth,
	totalWidth,
	scrollRef,
	viewportId,
	minimumVisibleFraction,
	onScrollLeftChange,
	onVisibleRangeChange,
}: {
	definition: TimecodeDefinition;
	items: readonly TimelineItem[];
	duration: number;
	scrollLeft: number;
	viewportWidth: number;
	totalWidth: number;
	scrollRef: RefObject<HTMLDivElement | null>;
	viewportId: string;
	minimumVisibleFraction: number;
	onScrollLeftChange(scrollLeft: number): void;
	onVisibleRangeChange(startFraction: number, endFraction: number): void;
}) {
	const maximumScroll = Math.max(0, totalWidth - viewportWidth);
	const visibleWidth = Math.min(100, (viewportWidth / totalWidth) * 100);
	const visibleLeft = Math.min(
		100 - visibleWidth,
		Math.max(0, (scrollLeft / totalWidth) * 100),
	);
	const visibleStart = visibleLeft / 100;
	const visibleEnd = (visibleLeft + visibleWidth) / 100;
	const drag = useRef<{
		pointerId: number;
		mode: "start" | "end" | "pan";
		grabOffset: number;
	} | null>(null);
	const fractionAt = (clientX: number, overview: HTMLElement) => {
		const bounds = overview.getBoundingClientRect();
		return Math.max(
			0,
			Math.min(1, (clientX - bounds.left) / Math.max(1, bounds.width)),
		);
	};
	const panTo = (leftFraction: number) => {
		const next = Math.max(
			0,
			Math.min(maximumScroll, leftFraction * totalWidth),
		);
		if (scrollRef.current) scrollRef.current.scrollLeft = next;
		onScrollLeftChange(next);
	};
	const beginDrag = (
		event: ReactPointerEvent<HTMLDivElement>,
		overview: HTMLElement,
	) => {
		const fraction = fractionAt(event.clientX, overview);
		const edge =
			event.target instanceof Element
				? event.target.closest<HTMLElement>("[data-overview-edge]")?.dataset
						.overviewEdge
				: undefined;
		if (edge === "start" || edge === "end") {
			drag.current = {
				pointerId: event.pointerId,
				mode: edge,
				grabOffset: 0,
			};
			return;
		}
		const inside = fraction >= visibleStart && fraction <= visibleEnd;
		const grabOffset = inside ? fraction - visibleStart : visibleWidth / 200;
		drag.current = {
			pointerId: event.pointerId,
			mode: "pan",
			grabOffset,
		};
		panTo(fraction - grabOffset);
	};
	const moveDrag = (clientX: number, overview: HTMLElement) => {
		const current = drag.current;
		if (!current) return;
		const fraction = fractionAt(clientX, overview);
		if (current.mode === "start") {
			onVisibleRangeChange(
				Math.max(0, Math.min(visibleEnd - minimumVisibleFraction, fraction)),
				visibleEnd,
			);
			return;
		}
		if (current.mode === "end") {
			onVisibleRangeChange(
				visibleStart,
				Math.min(1, Math.max(visibleStart + minimumVisibleFraction, fraction)),
			);
			return;
		}
		panTo(fraction - current.grabOffset);
	};
	const lanes = [
		...(definition.audio ? [{ id: "main-audio", kind: "audio" }] : []),
		...definition.lanes.map((lane) => ({
			id: lane.id,
			kind: lane.content.kind,
		})),
	];
	const laneCount = Math.max(1, lanes.length);
	const laneHeight = Math.max(1, Math.min(3, Math.floor(44 / laneCount)));
	const laneGap = laneHeight * laneCount + laneCount - 1 <= 44 ? 1 : 0;
	return (
		<div
			className="timecode-timeline-overview"
			role="scrollbar"
			aria-label="Timeline overview"
			aria-controls={viewportId}
			aria-orientation="horizontal"
			aria-valuemin={0}
			aria-valuemax={Math.round(maximumScroll)}
			aria-valuenow={Math.round(scrollLeft)}
			tabIndex={0}
			onPointerDown={(event) => {
				event.currentTarget.setPointerCapture(event.pointerId);
				beginDrag(event, event.currentTarget);
			}}
			onPointerMove={(event) => {
				if (event.currentTarget.hasPointerCapture(event.pointerId))
					moveDrag(event.clientX, event.currentTarget);
			}}
			onPointerUp={(event) => {
				if (drag.current?.pointerId === event.pointerId) drag.current = null;
			}}
			onPointerCancel={(event) => {
				if (drag.current?.pointerId === event.pointerId) drag.current = null;
			}}
			onKeyDown={(event) => {
				if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") return;
				event.preventDefault();
				const direction = event.key === "ArrowRight" ? 1 : -1;
				if (scrollRef.current)
					panTo(
						(scrollRef.current.scrollLeft + direction * viewportWidth * 0.1) /
							totalWidth,
					);
			}}
		>
			<div
				className="timecode-timeline-overview-lanes"
				style={
					{
						"--timecode-overview-lane-height": `${laneHeight}px`,
						"--timecode-overview-lane-gap": `${laneGap}px`,
					} as CSSProperties
				}
				data-lane-height={laneHeight}
				aria-hidden="true"
			>
				{lanes.map((lane) => (
					<span
						key={lane.id}
						className={`timecode-timeline-overview-lane overview-lane-${lane.kind}`}
					>
						{items
							.filter((item) => item.laneId === lane.id)
							.map((item) => (
								<i
									key={item.selection.itemId}
									style={{
										left: `${(item.frame / duration) * 100}%`,
										width: item.endFrame
											? `${Math.max(0.25, ((item.endFrame - item.frame) / duration) * 100)}%`
											: undefined,
									}}
								/>
							))}
					</span>
				))}
			</div>
			{definition.markers.map((marker) => (
				<i
					key={marker.id}
					className="timecode-timeline-overview-marker"
					style={{
						left: `${(marker.frame / duration) * 100}%`,
						background: markerColorOption(marker.color).value,
					}}
					aria-hidden="true"
				/>
			))}
			<span
				className="timecode-timeline-overview-visible"
				style={{ left: `${visibleLeft}%`, width: `${visibleWidth}%` }}
			>
				<i
					className="timecode-timeline-overview-handle handle-start"
					data-overview-edge="start"
					role="separator"
					aria-label="Resize timeline overview from start"
					aria-orientation="vertical"
				/>
				<i
					className="timecode-timeline-overview-handle handle-end"
					data-overview-edge="end"
					role="separator"
					aria-label="Resize timeline overview from end"
					aria-orientation="vertical"
				/>
			</span>
		</div>
	);
}

function EditorLane(
	props: Parameters<typeof TimelineCanvas>[0] & {
		lane: TimecodeDefinition["lanes"][number];
	},
) {
	const { lane } = props;
	const speedPositions =
		lane.content.kind === "speed_group"
			? speedKeyframePositions(lane.content.keyframes)
			: new Map<string, number>();
	const cueListId =
		lane.content.kind === "cue_list" ? lane.content.cue_list_id : null;
	const cueList =
		cueListId !== null
			? props.cueLists.find((candidate) => candidate.id === cueListId)
			: undefined;
	return (
		<div
			className={`timecode-editor-lane lane-${lane.content.kind} ${props.selectedLaneId === lane.id ? "selected" : ""}`}
			data-lane-id={lane.id}
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
					<AudioLaneFileName name={props.definition.audio?.file_name} />
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
			{lane.content.kind === "speed_group" &&
				lane.content.keyframes.length > 0 && (
					<svg
						className="timecode-speed-keyframe-curve"
						viewBox="0 0 100 100"
						preserveAspectRatio="none"
						role="img"
						aria-label={`${lane.name} value line`}
					>
						<polyline
							points={speedGroupLinePoints(
								lane.content.keyframes,
								props.duration,
							)}
						/>
					</svg>
				)}
			<LaneLabel
				lane={lane}
				audioFileName={props.definition.audio?.file_name}
				startLaneDrag={props.startLaneDrag}
				consumeLaneDragClick={props.consumeLaneDragClick}
				onSelectLane={props.onSelectLane}
			/>
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
						onSelect={props.onSelectItem}
						verticalPosition={
							item.kind === "speed"
								? speedPositions.get(item.selection.itemId)
								: undefined
						}
						startVolume={
							item.kind === "volume" && lane.content.kind === "audio_volume"
								? lane.content.keyframes.find(
										(keyframe) => keyframe.id === item.selection.itemId,
									)?.value
								: undefined
						}
						cueList={cueList}
						cueClip={
							lane.content.kind === "cue_list" && item.kind === "clip"
								? lane.content.clips.find(
										(clip) => clip.id === item.selection.itemId,
									)
								: undefined
						}
						timingDefaults={props.timingDefaults}
						onSaveCueList={props.onSaveCueList}
						onPlaceCueStart={(clipId, cueId, offsetFrame) =>
							props.onPlaceCueStart(lane.id, clipId, cueId, offsetFrame)
						}
						onSetClipFade={(clipId, kind, frames) =>
							props.onSetClipFade(lane.id, clipId, kind, frames)
						}
						onCueTimingError={props.onCueTimingError}
						clipStatus={
							lane.content.kind === "cue_list" && item.kind === "clip"
								? props.clipStatuses.find(
										(status) =>
											status.lane_id === lane.id &&
											status.clip_id === item.selection.itemId &&
											status.cue_list_id === cueListId,
									)
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
	onSelect,
	startVolume,
	verticalPosition,
	markersLocked = false,
	cueList,
	cueClip,
	timingDefaults,
	onSaveCueList,
	onPlaceCueStart,
	onSetClipFade,
	onCueTimingError,
	clipStatus,
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
	onSelect(selection: TimecodeEditorSelection): void;
	startVolume?: number;
	verticalPosition?: number;
	markersLocked?: boolean;
	cueList?: TimecodeCueListOption;
	cueClip?: TimecodeCueListClip;
	timingDefaults?: CueClipTimingDefaults;
	onSaveCueList?(cueListId: string, body: CueList): Promise<CueList>;
	onPlaceCueStart?(clipId: string, cueId: string, offsetFrame: number): void;
	onSetClipFade?(clipId: string, kind: ClipFadeKind, frames: number): void;
	onCueTimingError?(message: string): void;
	clipStatus?: TimecodeCueListClipStatus;
}) {
	const width = item.endFrame
		? Math.max(44, (item.endFrame - item.frame) * pixelsPerFrame)
		: undefined;
	const markerColor = marker ? markerColorOption(item.color) : null;
	const isCueListClip = item.kind === "clip" && item.endFrame !== undefined;
	return (
		<Button
			className={`${marker ? "timecode-timeline-marker" : `timecode-timeline-item item-${item.kind}`} ${sameSelection(selection, item.selection) ? "selected" : ""}`}
			style={
				{
					left: timelineFrameX(item.frame, pixelsPerFrame),
					...(marker
						? {
								color: markerColor?.value,
								"--timecode-marker-color": markerColor?.value,
								"--timecode-marker-text-color": markerColor?.text,
							}
						: {}),
					...(verticalPosition !== undefined
						? { top: `${verticalPosition}%` }
						: {}),
					...(width ? { width } : {}),
				} as CSSProperties
			}
			onPointerDown={(event) => {
				if (marker && markersLocked) {
					event.preventDefault();
					onSelect(item.selection);
					return;
				}
				// A Cuelist clip is dragged by its handle alone, because the body of the clip
				// belongs to the Cue sub-clips the operator edits inside it.
				if (isCueListClip) {
					onSelect(item.selection);
					return;
				}
				startDrag(event, item.selection, item.frame, undefined, startVolume);
			}}
			aria-disabled={marker && markersLocked ? true : undefined}
			title={`${item.label} · ${formatFrame(item.frame, fps)}`}
		>
			{clipStatus && <CueListClipStatus status={clipStatus} />}
			{isCueListClip && cueClip && (
				<CueListClipBody
					item={item}
					clip={cueClip}
					cueList={cueList}
					timingDefaults={timingDefaults}
					pixelsPerFrame={pixelsPerFrame}
					startDrag={startDrag}
					startVolume={startVolume}
					onSaveCueList={onSaveCueList}
					onPlaceCueStart={onPlaceCueStart}
					onSetClipFade={onSetClipFade}
					onCueTimingError={onCueTimingError}
				/>
			)}
			{marker ? (
				<>
					<span className="timecode-timeline-marker-line" aria-hidden="true" />
					<span className="timecode-timeline-marker-label">{item.label}</span>
				</>
			) : isCueListClip ? null : (
				(item.valueLabel ?? item.label)
			)}
			{!marker && !isCueListClip && <small>{formatFrame(item.frame, fps)}</small>}
		</Button>
	);
}


type SpeedKeyframe = Extract<
	TimecodeDefinition["lanes"][number]["content"],
	{ kind: "speed_group" }
>["keyframes"][number];

function speedKeyframePositions(
	keyframes: readonly SpeedKeyframe[],
): Map<string, number> {
	if (!keyframes.length) return new Map();
	const bpms = keyframes.map((keyframe) => keyframe.bpm);
	const minimum = Math.min(...bpms);
	const maximum = Math.max(...bpms);
	const span = maximum - minimum;
	return new Map(
		keyframes.map((keyframe) => [
			keyframe.id,
			span === 0 ? 50 : 88 - ((keyframe.bpm - minimum) / span) * 76,
		]),
	);
}

export function speedGroupLinePoints(
	keyframes: readonly SpeedKeyframe[],
	durationFrames: number,
): string {
	const sorted = [...keyframes].sort((left, right) => left.frame - right.frame);
	const first = sorted[0];
	if (!first) return "";
	const positions = speedKeyframePositions(sorted);
	const duration = Math.max(1, durationFrames);
	const point = (frame: number, keyframe: SpeedKeyframe) =>
		`${Math.max(0, Math.min(100, (frame / duration) * 100))},${positions.get(keyframe.id) ?? 50}`;
	const points = [point(0, first), point(first.frame, first)];
	for (let index = 1; index < sorted.length; index += 1) {
		const previous = sorted[index - 1];
		const current = sorted[index];
		if (!previous || !current) continue;
		points.push(point(current.frame, previous), point(current.frame, current));
	}
	const last = sorted.at(-1);
	if (last) points.push(point(duration, last));
	return points.join(" ");
}


const TARGET_MAX_PIXELS_PER_FRAME = 17.5;
const FALLBACK_VIEWPORT_WIDTH = 720;
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

function useLaneReorder({
	definition,
	onPreview,
	onBeginGesture,
	onEndGesture,
}: Pick<
	Props,
	"definition" | "onPreview" | "onBeginGesture" | "onEndGesture"
>) {
	const latest = useRef(definition);
	latest.current = definition;
	const drag = useRef<{
		laneId: string;
		pointerId: number;
		startY: number;
		active: boolean;
	} | null>(null);
	const suppressedClick = useRef<string | null>(null);
	useEffect(() => {
		const move = (event: PointerEvent) => {
			const current = drag.current;
			if (!current || event.pointerId !== current.pointerId) return;
			if (!current.active && Math.abs(event.clientY - current.startY) < 8)
				return;
			if (!current.active) {
				current.active = true;
				suppressedClick.current = current.laneId;
				onBeginGesture();
			}
			const targetLane = document
				.elementFromPoint(event.clientX, event.clientY)
				?.closest<HTMLElement>(".timecode-editor-lane")?.dataset.laneId;
			if (!targetLane || targetLane === current.laneId) return;
			const next = reorderTimelineLane(
				latest.current,
				current.laneId,
				targetLane,
			);
			if (next === latest.current) return;
			latest.current = next;
			onPreview(next);
		};
		const finish = (event: PointerEvent) => {
			const current = drag.current;
			if (!current || event.pointerId !== current.pointerId) return;
			drag.current = null;
			if (current.active) onEndGesture();
		};
		window.addEventListener("pointermove", move);
		window.addEventListener("pointerup", finish);
		window.addEventListener("pointercancel", finish);
		return () => {
			window.removeEventListener("pointermove", move);
			window.removeEventListener("pointerup", finish);
			window.removeEventListener("pointercancel", finish);
		};
	}, [onBeginGesture, onEndGesture, onPreview]);
	return {
		start: (event: ReactPointerEvent, laneId: string) => {
			if (event.button !== 0) return;
			drag.current = {
				laneId,
				pointerId: event.pointerId,
				startY: event.clientY,
				active: false,
			};
		},
		consumeClick: (laneId: string) => {
			if (suppressedClick.current !== laneId) return false;
			suppressedClick.current = null;
			return true;
		},
	};
}

export function timelineZoomGeometry(
	durationFrames: number,
	viewportWidth: number,
) {
	const duration = Math.max(1, durationFrames);
	const viewport = Math.max(1, viewportWidth);
	const fit = Math.min(TARGET_MAX_PIXELS_PER_FRAME, viewport / duration);
	return {
		fitPixelsPerFrame: fit,
		maximumZoom: Math.max(
			1,
			Math.ceil((TARGET_MAX_PIXELS_PER_FRAME / fit) * 4) / 4,
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
		clipStatuses = [],
		timingDefaults = {
			sequenceFadeMillis: 3_000,
			releaseFadeMillis: 3_000,
		},
		onScrub,
		onCommit,
		onPreview,
		onBeginGesture,
		onEndGesture,
		onSaveCueList,
		onCueTimingError,
	},
	ref,
) {
	type Selection = TimecodeEditorSelection | null;
	const [selection, setSelection] = useState<Selection>(null);
	const [selectedCueId, setSelectedCueId] = useSelectedCue(selection);
	const [selectedLaneId, setSelectedLaneId] = useState<string | null>(null);
	const [speedGroup, setSpeedGroup] = useState("A");
	const [speedGroupChooserOpen, setSpeedGroupChooserOpen] = useState(false);
	const [cueListChooserOpen, setCueListChooserOpen] = useState(false);
	const [cueListId, setCueListId] = useState(cueLists[0]?.id ?? "");
	const [scrollLeft, setScrollLeft] = useState(0);
	const [zoom, setZoom] = useState(1);
	const [overviewResize, setOverviewResize] =
		useState<OverviewResize | null>(null);
	const viewportId = useId();
	const scrollRef = useRef<HTMLDivElement>(null);
	const scrollTimelineTo = timelineScroller(scrollRef, setScrollLeft);
	const viewportWidth = useTimelineViewportWidth(scrollRef);
	const duration = Math.max(1, definition.duration_frame ?? fps * 60);
	const timelineViewportWidth = Math.max(1, viewportWidth - TIMECODE_LANE_HEADER_WIDTH);
	const { maximumZoom, fitPixelsPerFrame } = timelineZoomGeometry(
		duration,
		timelineViewportWidth,
	);
	const pixelsPerFrame = Math.min(
		TARGET_MAX_PIXELS_PER_FRAME,
		fitPixelsPerFrame * zoom,
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
	const activeMarker =
		selection?.kind === "marker"
			? definition.markers.find((marker) => marker.id === selection.itemId)
			: undefined;
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
		onScaleCueTimings: (selection, ratio) =>
			scaleClipCueTimings({
				definition,
				cueLists,
				selection,
				ratio,
				onSaveCueList,
				onCueTimingError,
			}),
	});
	const laneReorder = useLaneReorder({
		definition,
		onPreview,
		onBeginGesture,
		onEndGesture,
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
		timingDefaults,
		onCommit,
		setSelection,
		setSelectedLane: setSelectedLaneId,
	});
	useEffect(() => {
		if (zoom > maximumZoom) setZoom(maximumZoom);
	}, [maximumZoom, zoom]);
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
	useLayoutEffect(() => {
		const viewport = scrollRef.current;
		if (!viewport) return;
		const maximumScroll = Math.max(0, (width + TIMECODE_LANE_HEADER_WIDTH) - viewportWidth);
		if (overviewResize) {
			const next = Math.max(
				0,
				Math.min(maximumScroll, overviewResize.startFraction * width),
			);
			viewport.scrollLeft = next;
			setScrollLeft(next);
			setOverviewResize(null);
			return;
		}
		if (viewport.scrollLeft > maximumScroll)
			viewport.scrollLeft = maximumScroll;
		setScrollLeft(viewport.scrollLeft);
	}, [overviewResize, (width + TIMECODE_LANE_HEADER_WIDTH), viewportWidth, width]);
	const resizeOverviewWindow = (startFraction: number, endFraction: number) => {
		const requestedFraction = Math.max(0.0001, endFraction - startFraction);
		const requestedPixelsPerFrame = Math.min(
			TARGET_MAX_PIXELS_PER_FRAME,
			timelineViewportWidth / requestedFraction / duration,
		);
		const nextZoom = Math.max(
			1,
			Math.min(maximumZoom, requestedPixelsPerFrame / fitPixelsPerFrame),
		);
		setOverviewResize((current) => ({
			zoom: nextZoom,
			startFraction,
			revision: (current?.revision ?? 0) + 1,
		}));
		setZoom(nextZoom);
	};
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
	// A selected Cuelist clip takes the encoder deck over, so the four Cue timings and the Cue
	// itself sit on the encoders instead of the keyframe slots.
	const cueContext = useCueEncoderContext({
		activeLane,
		selection,
		cueLists,
		selectedCueId,
		setSelectedCueId,
		timingDefaults,
		onSaveCueList,
	});
	useTimecodeEncoderSlots({
		definition,
		cueContext,
		items,
		keyframeItems,
		selection,
		activeLane,
		activeKeyframe,
		activeMarker,
		speedKeyframe,
		volumeKeyframe,
		laneIndex,
		keyframeIndex,
		duration,
		frame,
		fps,
		zoom,
		maximumZoom,
		scrollLeft,
		viewportWidth,
		timelineWidth: width,
		encoderOwner,
		setZoom,
		setScrollLeft: scrollTimelineTo,
		setSelection,
		setSelectedLaneId,
		onScrub,
		onCommit,
	});
	useEffect(() => () => clearTimecodeEncoderDeck(encoderOwner), [encoderOwner]);

	const { placeCueStart, setClipFade } = clipEditors(definition, onCommit);

	return (
		<section
			className="timecode-timeline-editor"
			aria-label="Timecode timeline editor"
		>
			<TimelineOverview
				definition={definition}
				items={items}
				duration={duration}
				scrollLeft={scrollLeft}
				viewportWidth={timelineViewportWidth}
				totalWidth={width}
				scrollRef={scrollRef}
				viewportId={viewportId}
				minimumVisibleFraction={Math.min(
					1,
					timelineViewportWidth / (duration * TARGET_MAX_PIXELS_PER_FRAME),
				)}
				onScrollLeftChange={setScrollLeft}
				onVisibleRangeChange={resizeOverviewWindow}
			/>
			<TimelineCanvas
				{...{
					definition,
					frame,
					fps,
					waveformPeaks,
					markersLocked,
					clipStatuses,
					duration,
					width,
					pixelsPerFrame,
					items,
					selection,
					scrollRef,
					onScrub,
					startDrag,
					startLaneDrag: laneReorder.start,
					consumeLaneDragClick: laneReorder.consumeClick,
					onSelectItem: setSelection,
					onScroll: setScrollLeft,
					viewportId,
					onSelectLane: (laneId: string) => {
						setSelectedLaneId(laneId);
						const first = items.find((item) => item.laneId === laneId);
						setSelection(first?.selection ?? null);
					},
					selectedLaneId: activeLaneId,
					cueLists,
					timingDefaults,
					onSaveCueList,
					onPlaceCueStart: placeCueStart,
					onSetClipFade: setClipFade,
					onCueTimingError,
				}}
			/>
			<KeyframeActionStrip
				definition={definition}
				selection={selection}
				laneId={activeLaneId}
				frame={frame}
				fps={fps}
				items={items}
				cueLists={cueLists}
				selectedCueId={selectedCueId}
				onSelectCue={setSelectedCueId}
				onSelection={(item) => setSelection(item.selection)}
				onInsert={() => {
					if (activeLaneId) addKeyframe(activeLaneId, frame);
				}}
				onCommit={onCommit}
				onAddClip={() => {
					if (activeLaneId) addClip(activeLaneId);
				}}
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

/// Replaces the easing curve of one audio-volume keyframe.

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
			cueLists
				.find((candidate) => candidate.id === content.cue_list_id)
				?.cues.flatMap((cue) => (cue.id ? [{ ...cue, id: cue.id }] : [])) ?? [];
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
			<MarkerColorButton
				color={marker.color}
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
				min={SPEED_GROUP_MIN_BPM}
				max={SPEED_GROUP_MAX_BPM}
				step={0.1}
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
	// Offering only Cues that keep the clip order valid removes the "end Cue is
	// before its start Cue" error instead of reporting it after the fact.
	const startIndex = Math.max(
		0,
		cues.findIndex((cue) => cue.id === clip.start_cue_id),
	);
	const endIndex =
		cues.findIndex((cue) => cue.id === clip.end_cue_id) < 0
			? cues.length - 1
			: cues.findIndex((cue) => cue.id === clip.end_cue_id);
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
				cues={cues.slice(0, endIndex + 1)}
				onValue={(start_cue_id) => update({ start_cue_id })}
			/>
			<CueSelect
				label="End Cue"
				value={clip.end_cue_id}
				cues={cues.slice(startIndex)}
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
							tick === 0
								? "timecode-ruler-first-tick"
								: tick === duration
									? "timecode-ruler-final-tick"
									: undefined
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

/// The clip edits the timeline canvas hands back, each one a whole new definition.
///
/// `placeCueStart` stores a lane-owned transition point for a Cue that waits for a manual GO;
/// `setClipFade` stores a clip's own in or out fade, which shapes the level it contributes.
function clipEditors(
	definition: TimecodeDefinition,
	onCommit: (definition: TimecodeDefinition) => void,
) {
	return {
		placeCueStart: (
			laneId: string,
			clipId: string,
			cueId: string,
			offsetFrame: number,
		) =>
			onCommit(
				withPlacedCueStart(definition, laneId, clipId, cueId, offsetFrame),
			),
		setClipFade: (
			laneId: string,
			clipId: string,
			kind: ClipFadeKind,
			frames: number,
		) => onCommit(withClipFade(definition, laneId, clipId, kind, frames)),
	};
}
