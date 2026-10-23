import { useTimelineInspectorPreference } from "./useTimecodeSidebarPreference";
import { useTimelineInspectorVisibility } from "./useTimelineInspectorVisibility";
import { volumeKeyframeValues } from "./audioLaneGeometry";
import { useTimecodeEditorHandle } from "./useTimecodeEditorHandle";
import { cueStatusForItem } from "./cueLaneClipStatus";
import { overviewResizer } from "./timelineOverviewResize";
import { createCueClipEndWriter } from "./extendCueClipEnd";
import {
	useTimelineSections,
	useTimelineSelectedLane,
} from "./useTimelineSections";
import { useExpandedLaneVisibility } from "./useExpandedLaneVisibility";
import { useCueLaneView } from "./useCueLaneView";
import { TimelineOverview } from "./TimecodeTimelineOverview";
import {
	TIMECODE_LANE_SECTIONS,
	TimecodeLaneSection,
} from "./TimecodeLaneSection";
import {
	speedKeyframePositions,
	speedGroupLinePoints,
} from "./speedLaneGeometry";
export { speedGroupLinePoints } from "./speedLaneGeometry";
import { Ruler } from "./TimecodeRuler";
import { timelineSelectionContext } from "./timelineSelectionContext";
import {
	useTimelineGeometry,
	TARGET_MAX_PIXELS_PER_FRAME,
} from "./useTimelineGeometry";
export { timelineZoomGeometry } from "./useTimelineGeometry";
import { useCueClipBoundaryResize } from "./useCueClipBoundaryResize";
import {
	useTimelineTitleActions,
	type TimelineTitleState,
} from "./useTimelineTitleActions";
import {
	CueClipBoundaryProvider,
	cueClipBoundarySlots,
} from "./CueClipBoundaryContext";
import { SelectionSidebar } from "./TimecodeSelectionInspector";
import { Button } from "@tosklight/ui";
import {
	type CSSProperties,
	forwardRef,
	type PointerEvent as ReactPointerEvent,
	type RefObject,
	useEffect,
	useId,
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
import { type CueClipTimingDefaults } from "./cueClipTiming";
import {
	reorderTimelineLane,
	moveTimelineItem,
	sameSelection,
	type TimecodeEditorSelection,
	timelineItems,
	withClipFade,
	withPlacedCueStart,
} from "./editorModel";
import { type ClipFadeKind, CueListClipBody } from "./TimecodeClipBody";
import {
	LaneLabel,
	AudioVolumeLaneBackground,
	SpeedGroupLaneBackground,
	lanePointerSelection,
} from "./TimecodeLaneParts";
import { CueListClipStatus } from "./TimecodeCueClipContents";
import {
	scaleClipCueTimings,
	cueRangeOfSelectedClip,
	useCueEncoderContext,
	useSelectedCue,
} from "./TimecodeKeyframeActions";
import { TimelineLaneChoosers } from "./TimecodeLaneChoosers";
import {
	formatFrame,
	displayedTimecodeLanes,
	markerColorOption,
	TIMECODE_LANE_HEADER_WIDTH,
	timelineScroller,
	type OverviewResize,
	type TimecodeAudioPlayerOption,
	type TimecodeCueListOption,
	type TimelineItem,
	timelineFrameX,
} from "./timecodeEditorShared";
import {
	TIMECODE_SPEED_GROUPS,
} from "./TimecodeSpeedGroupChooser";
import { useTimelineEncoderOwner } from "./useTimelineEncoderOwner";
import { useTimecodeEncoderSlots } from "./timecodeEncoderSlots";
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
	stepKeyframe(delta: -1 | 1): void;
	deleteSelection(): void;
	insertKeyframe(): void;
	addClip(): void;
	fit(): void;
	addMarker(): void;
	chooseSpeedLane(): void;
	chooseCueListLane(): void;
	addAudioPlayerLane(fixtureId: string): void;
}

function TimelineCanvas(props: {
	collapsedSections: Record<string, boolean>;
	onToggleSection(kind: string): void;
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
		frame: number,
		clipEdge?: "start" | "end",
	): void;
	startLaneDrag(event: ReactPointerEvent, laneId: string): void;
	consumeLaneDragClick(laneId: string): boolean;
	onSelectItem(selection: TimecodeEditorSelection): void;
	selectedCueId: string | null;
	onSelectCue(cueId: string, selection?: TimecodeEditorSelection): void;
	inspectorHost: HTMLElement | null;
	onMoveClip(laneId: string, clipId: string, frame: number): void;
	onSetClipEnd(laneId: string, clipId: string, frame: number): void;
	onCloseCue(): void;
	onSelectLane(laneId: string): void;
	onScroll(scrollLeft: number): void;
	selectedLaneId: string | null;
	expandedSpeedLaneId: string | null;
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
				{props.definition.lanes.length === 0 && (
					<p className="timecode-timeline-empty">
						Use Add to create a Cuelist or Speed lane. Choose audio in Timecode
						Settings.
					</p>
				)}
				<TimelineLanes {...props} />
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

function TimelineLanes(props: Parameters<typeof TimelineCanvas>[0]) {
	const displayedLanes = displayedTimecodeLanes(props.definition);
	return (
		<>
			{" "}
			{props.definition.audio &&
				displayedLanes
					.filter((lane) => lane.content.kind === "audio_volume")
					.map((lane) => <EditorLane key={lane.id} {...props} lane={lane} />)}
			{TIMECODE_LANE_SECTIONS.map((section) => (
				<TimecodeLaneSection
					key={section.kind}
					{...section}
					lanes={displayedLanes.filter(
						(lane) => lane.content.kind === section.kind,
					)}
					collapsed={Boolean(props.collapsedSections[section.kind])}
					duration={props.duration}
					onToggle={() => props.onToggleSection(section.kind)}
					renderLane={(lane) => (
						<EditorLane key={lane.id} {...props} lane={lane} />
					)}
				/>
			))}
			{!props.definition.audio &&
				displayedLanes
					.filter((lane) => lane.content.kind === "audio_volume")
					.map((lane) => <EditorLane key={lane.id} {...props} lane={lane} />)}
		</>
	);
}

function cueListForLane(
	lane: TimecodeDefinition["lanes"][number],
	cueLists: readonly TimecodeCueListOption[],
) {
	const id = lane.content.kind === "cue_list" ? lane.content.cue_list_id : null;
	return cueLists.find((candidate) => candidate.id === id);
}

function EditorLane(
	props: Parameters<typeof TimelineCanvas>[0] & {
		lane: TimecodeDefinition["lanes"][number];
	},
) {
	const cueList = cueListForLane(props.lane, props.cueLists);
	const {
		displayDefinition,
		displayItems,
		previewMoveClip,
		previewClipEnd,
		boundsForClip,
	} = useCueLaneView(
		props.definition,
		props.items,
		props.lane.id,
		cueList?.body,
		props.timingDefaults,
	);
	const lane =
		displayDefinition.lanes.find(
			(candidate) => candidate.id === props.lane.id,
		) ?? props.lane;
	const volumeValues = volumeKeyframeValues(lane.content);
	const speedPositions =
		lane.content.kind === "speed_group"
			? speedKeyframePositions(lane.content.keyframes)
			: new Map<string, number>();
	const cueListId =
		lane.content.kind === "cue_list" ? lane.content.cue_list_id : null;
	useExpandedLaneVisibility(
		props.scrollRef,
		props.selectedLaneId === lane.id ? lane.id : null,
		props.selectedCueId,
	);
	return (
		<div
			className={`timecode-editor-lane lane-${lane.content.kind} ${props.definition.audio && lane.content.kind === "audio_volume" ? "main-audio" : ""} ${props.selectedLaneId === lane.id ? "selected" : ""} ${props.expandedSpeedLaneId === lane.id && lane.content.kind === "speed_group" ? "lane-expanded" : ""} ${props.selectedLaneId === lane.id && props.selectedCueId ? "cue-expanded" : ""}`}
			data-lane-id={lane.id}
			onPointerDown={lanePointerSelection(props.onSelectLane, lane.id)}
		>
			{lane.content.kind === "audio_volume" && (
				<AudioVolumeLaneBackground
					content={lane.content}
					duration={props.duration}
					pixelsPerFrame={props.pixelsPerFrame}
					fileName={props.definition.audio?.file_name}
					waveformPeaks={props.waveformPeaks}
				/>
			)}
			{lane.content.kind === "speed_group" && (
				<SpeedGroupLaneBackground
					name={lane.name}
					content={lane.content}
					duration={props.duration}
					pixelsPerFrame={props.pixelsPerFrame}
				/>
			)}
			<LaneLabel
				lane={lane}
				pinned={Boolean(
					props.definition.audio && lane.content.kind === "audio_volume",
				)}
				audioFileName={props.definition.audio?.file_name}
				startLaneDrag={props.startLaneDrag}
				consumeLaneDragClick={props.consumeLaneDragClick}
				onSelectLane={props.onSelectLane}
				expanded={
					(props.selectedLaneId === lane.id && Boolean(props.selectedCueId)) || props.expandedSpeedLaneId === lane.id
				}
				onCloseCue={props.onCloseCue}
			/>
			{displayItems
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
						verticalPosition={item.kind === "speed"
							? speedPositions.get(item.selection.itemId)
							: volumeValues.get(item.selection.itemId)?.position}
						startVolume={volumeValues.get(item.selection.itemId)?.value}
						cueList={cueList}
						selectedCueId={
							sameSelection(props.selection, item.selection)
								? props.selectedCueId
								: null
						}
						onSelectCue={(cueId) => {
							props.onSelectItem(item.selection);
							props.onSelectCue(cueId, item.selection);
						}}
						onCloseCue={props.onCloseCue}
						inspectorHost={props.inspectorHost}
						onPreviewMoveClip={(clipId, frame) =>
							previewMoveClip(lane.id, clipId, frame)
						}
						clipEndBounds={boundsForClip(item.selection.itemId)}
						onPreviewClipEnd={previewClipEnd}
						onSetClipEnd={(clipId, frame) =>
							props.onSetClipEnd(lane.id, clipId, frame)
						}
						onMoveClip={(clipId, frame) =>
							props.onMoveClip(lane.id, clipId, frame)
						}
						expanded={
							sameSelection(props.selection, item.selection) &&
							Boolean(props.selectedCueId)
						}
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
						clipStatus={cueStatusForItem(
							props.clipStatuses,
							lane,
							item,
							cueListId,
						)}
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
	selectedCueId = null,
	onSelectCue,
	onCloseCue,
	expanded = false,
	inspectorHost = null,
	onMoveClip,
	onPreviewMoveClip,
	onSetClipEnd,
	onPreviewClipEnd,
	clipEndBounds,
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
	selectedCueId?: string | null;
	inspectorHost?: HTMLElement | null;
	onMoveClip?(clipId: string, frame: number): void;
	onPreviewMoveClip?(clipId: string, frame: number | null): void;
	onSetClipEnd?(clipId: string, frame: number): void;
	onPreviewClipEnd?(clipId: string, frame: number | null): void;
	clipEndBounds?: { minimum: number; maximum: number } | null;
	onSelectCue?(cueId: string): void;
	onCloseCue?(): void;
	expanded?: boolean;
}) {
	const width = item.endFrame
		? cueClip
			? Math.max(0, (item.endFrame - item.frame) * pixelsPerFrame)
			: Math.max(44, (item.endFrame - item.frame) * pixelsPerFrame)
		: undefined;
	const markerColor = marker ? markerColorOption(item.color) : null;
	const isCueListClip = item.kind === "clip" && item.endFrame !== undefined;
	const ItemSurface = isCueListClip && cueClip ? "div" : Button;
	return (
		<ItemSurface
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
			title={`${cueClip ? (cueList?.name ?? "Cuelist") : item.label} · ${formatFrame(item.frame, fps)}`}
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
					selectedCueId={selectedCueId}
					onSelectCue={onSelectCue}
					onCloseCue={onCloseCue}
					expanded={expanded}
					inspectorHost={inspectorHost}
					onMoveClip={onMoveClip}
					onPreviewMoveClip={onPreviewMoveClip}
					onSetClipEnd={clipEndBounds ? onSetClipEnd : undefined}
					onPreviewClipEnd={clipEndBounds ? onPreviewClipEnd : undefined}
					clipEndMinimum={clipEndBounds?.minimum}
					clipEndMaximum={clipEndBounds?.maximum}
				/>
			)}
			{marker ? (
				<>
					<span className="timecode-timeline-marker-line" aria-hidden="true" />
					<span className="timecode-timeline-marker-label"><span className="timecode-marker-grab" aria-hidden="true" />{item.label}</span>
				</>
			) : isCueListClip ? null : (
				(item.valueLabel ?? item.label)
			)}
			{!marker && !isCueListClip && (
				<small>{formatFrame(item.frame, fps)}</small>
			)}
		</ItemSurface>
	);
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
			if (
				latest.current.audio &&
				latest.current.lanes.some(
					(lane) =>
						(lane.id === targetLane || lane.id === current.laneId) &&
						lane.content.kind === "audio_volume",
				)
			)
				return;
			if (
				latest.current.lanes.find((lane) => lane.id === targetLane)?.content
					.kind !==
				latest.current.lanes.find((lane) => lane.id === current.laneId)?.content
					.kind
			)
				return;
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

	const [cueInspectorHost, setCueInspectorHost] =
		useState<HTMLDivElement | null>(null);
	const [selectedCueId, setSelectedCueId] = useSelectedCue(selection);
	const [selectedLaneId, setSelectedLaneId] =
		useTimelineSelectedLane(selection);
	const [speedGroup, setSpeedGroup] = useState("A");
	const [speedGroupChooserOpen, setSpeedGroupChooserOpen] = useState(false);
	const [cueListChooserOpen, setCueListChooserOpen] = useState(false);
	const [cueListId, setCueListId] = useState(cueLists[0]?.id ?? "");
	const [scrollLeft, setScrollLeft] = useState(0);
	const [overviewResize, setOverviewResize] = useState<OverviewResize | null>(
		null,
	);
	const viewportId = useId();
	const scrollRef = useRef<HTMLDivElement>(null);
	const scrollTimelineTo = timelineScroller(scrollRef, setScrollLeft);
	const duration = Math.max(1, definition.duration_frame ?? fps * 60);
	const {
		viewportWidth,
		timelineViewportWidth,
		maximumZoom,
		fitPixelsPerFrame,
		pixelsPerFrame,
		width,
		zoom,
		setZoom,
		fitTimeline,
		prepareInspectorClose,
	} = useTimelineGeometry({
		scrollRef,
		duration,
		definitionId: definition.id,
		scrollTimelineTo,
	});
	const inspectorPreference = useTimelineInspectorPreference();
	const inspector = useTimelineInspectorVisibility(inspectorPreference.visible, inspectorPreference.setVisible, prepareInspectorClose);
	const items = useMemo(() => timelineItems(definition), [definition]);
	const activeLaneId =
		selection && "laneId" in selection ? selection.laneId : selectedLaneId;
	const { collapsedSections, onToggleSection, onSelectLane, onCloseCue, expandedSpeedLaneId } =
		useTimelineSections({
			definition,
			activeLaneId,
			items,
			setSelection,
			setSelectedLaneId,
			setSelectedCueId,
			prepareInspectorClose,
		});
	const encoderOwner = useTimelineEncoderOwner();
	const {
		keyframeItems,
		laneIndex,
		keyframeIndex,
		activeLane,
		activeKeyframe,
		activeMarker,
		speedKeyframe,
		volumeKeyframe,
	} = timelineSelectionContext(definition, items, selection, activeLaneId);

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
	const titleActions = useTimelineTitleActions({
		definition,
		selection,
		laneId: activeLaneId,
		items,
		onSelect: setSelection,
		onCommit,
		onScrub,
	});
	useTimecodeEditorHandle(ref, { titleActions, fitTimeline, activeLaneId, frame, addKeyframe, addClip, addMarker, setSpeedGroupChooserOpen, setCueListChooserOpen, addAudioPlayerLane });

	useOverviewScroll({
		scrollRef,
		width,
		viewportWidth,
		overviewResize,
		setScrollLeft,
		setOverviewResize,
	});
	const resizeOverviewWindow = overviewResizer({
		timelineViewportWidth,
		duration,
		maximumZoom,
		fitPixelsPerFrame,
		setOverviewResize,
		setZoom,
	});

	useEffect(() => {
		if (!cueLists.some((cueList) => cueList.id === cueListId))
			setCueListId(cueLists[0]?.id ?? "");
	}, [cueListId, cueLists]);
	const availableSpeedGroups = unusedSpeedGroups(definition);
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

	const { placeCueStart, setClipFade, moveClip } = clipEditors(
		definition,
		onCommit,
	);

	const cueTimingActive = cueRangeOfSelectedClip(
		activeLane,
		selection,
		cueLists,
	).some((cue) => cue.id === selectedCueId);
	useEffect(() => {
		if (selectedCueId && !cueTimingActive) setSelectedCueId(null);
	}, [selectedCueId, cueTimingActive]);
	const resizeClip = useCueClipBoundaryResize({
		definition,
		cueLists,
		onBeginGesture,
		onPreview,
		onEndGesture,
		onSaveCueList,
		onCueTimingError,
	});

	return (
		<section
			className={`timecode-timeline-editor${cueTimingActive ? " cue-focus" : ""}`}
			aria-label="Timecode timeline editor"
		>
			<CueClipBoundaryProvider
				value={cueClipBoundarySlots({
					definition,
					selection,
					onResizeClip: resizeClip,
				})}
			>
				<div className="timecode-editor-body">
					<div className="timecode-editor-main">
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
								timelineViewportWidth /
									(duration * TARGET_MAX_PIXELS_PER_FRAME),
							)}
							onScrollLeftChange={setScrollLeft}
							onVisibleRangeChange={resizeOverviewWindow}
						/>
						<TimelineCanvas
							{...{
								definition,
								collapsedSections,
								onToggleSection,
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
								selectedCueId,
								onSelectCue: setSelectedCueId,
								inspectorHost: cueInspectorHost,
								onMoveClip: moveClip,
								onSetClipEnd: createCueClipEndWriter({
									definition,
									cueLists,
									timingDefaults,
									onCommit,
								}),
								onCloseCue,
								onScroll: setScrollLeft,
								viewportId,
								onSelectLane,
								selectedLaneId: activeLaneId,
								expandedSpeedLaneId,
								cueLists,
								timingDefaults,
								onSaveCueList,
								onPlaceCueStart: placeCueStart,
								onSetClipFade: setClipFade,
								onCueTimingError,
							}}
						/>
					</div>
					{inspector.visible && <SelectionSidebar
						{...{ definition, selection, cueLists, fps, items, onCommit, frame, onScrub }}
						onSelect={setSelection}
						beforeClose={prepareInspectorClose}
						cueTimingActive={cueTimingActive}
						inspectorRef={setCueInspectorHost}
						onClose={inspector.close}
					/>}
				</div>
			</CueClipBoundaryProvider>
			<TimelineLaneChoosers
				{...{
					cueListChooserOpen,
					cueLists,
					cueListId,
					setCueListId,
					setCueListChooserOpen,
					addCueListLane,
					speedGroupChooserOpen,
					availableSpeedGroups,
					speedGroup,
					setSpeedGroup,
					setSpeedGroupChooserOpen,
					addSpeedLane,
				}}
			/>
		</section>
	);
});

function unusedSpeedGroups(definition: TimecodeDefinition) {
	return TIMECODE_SPEED_GROUPS.filter(
		(group) =>
			!definition.lanes.some(
				(lane) =>
					lane.content.kind === "speed_group" && lane.content.group === group,
			),
	);
}

function useOverviewScroll({
	scrollRef,
	width,
	viewportWidth,
	overviewResize,
	setScrollLeft,
	setOverviewResize,
}: {
	scrollRef: RefObject<HTMLDivElement | null>;
	width: number;
	viewportWidth: number;
	overviewResize: OverviewResize | null;
	setScrollLeft(value: number): void;
	setOverviewResize(value: null): void;
}) {
	useLayoutEffect(() => {
		const viewport = scrollRef.current;
		if (!viewport) return;
		const maximumScroll = Math.max(
			0,
			width + TIMECODE_LANE_HEADER_WIDTH - viewportWidth,
		);
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
	}, [
		overviewResize,
		width + TIMECODE_LANE_HEADER_WIDTH,
		viewportWidth,
		width,
	]);
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
		moveClip: (laneId: string, clipId: string, frame: number) =>
			onCommit(
				moveTimelineItem(
					definition,
					{ kind: "clip", laneId, itemId: clipId },
					frame,
				),
			),
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
