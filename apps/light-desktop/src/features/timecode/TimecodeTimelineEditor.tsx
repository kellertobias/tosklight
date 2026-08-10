import {
	Button,
	ColorPickerField,
	HorizontalFader,
	NumberField,
	SelectField,
	TextAreaField,
	TextField,
} from "@tosklight/ui";
import {
	type PointerEvent as ReactPointerEvent,
	useMemo,
	useState,
} from "react";
import type { TimecodeDefinition } from "../../api/types/timecode";
import {
	copyTimelineItem,
	deleteTimelineItem,
	sameSelection,
	type TimecodeEditorSelection,
	timelineItems,
} from "./editorModel";
import {
	useTimelineActions,
	useTimelineDrag,
} from "./useTimecodeTimelineActions";

interface Props {
	definition: TimecodeDefinition;
	frame: number;
	fps: number;
	cueLists: readonly TimecodeCueListOption[];
	waveformPeaks?: readonly number[];
	onScrub(frame: number): void;
	onCommit(definition: TimecodeDefinition): void;
	onPreview(definition: TimecodeDefinition): void;
	onBeginGesture(): void;
	onEndGesture(): void;
}

function TimelineTools(props: {
	definition: TimecodeDefinition;
	selection: TimecodeEditorSelection | null;
	speedGroup: string;
	setSpeedGroup(value: string): void;
	cueListId: string;
	setCueListId(value: string): void;
	cueLists: readonly TimecodeCueListOption[];
	zoom: number;
	setZoom(value: number): void;
	addMarker(): void;
	addAudioLane(): void;
	addSpeedLane(): void;
	addCueListLane(): void;
	setCsvOpen(update: (open: boolean) => boolean): void;
	setSelection(value: TimecodeEditorSelection | null): void;
	onCommit(value: TimecodeDefinition): void;
	fps: number;
}) {
	const {
		definition,
		selection,
		speedGroup,
		setSpeedGroup,
		cueListId,
		setCueListId,
		cueLists,
		zoom,
		setZoom,
	} = props;
	const copy = () => {
		if (!selection) return;
		const copied = copyTimelineItem(
			definition,
			selection,
			crypto.randomUUID(),
			props.fps,
		);
		props.onCommit(copied.definition);
		props.setSelection(copied.selection);
	};
	const remove = () => {
		if (!selection) return;
		props.onCommit(deleteTimelineItem(definition, selection));
		props.setSelection(null);
	};
	return (
		<div className="timecode-timeline-tools">
			<Button onClick={props.addMarker}>Add marker at playhead</Button>
			<Button
				onClick={props.addAudioLane}
				disabled={definition.lanes.some(
					(lane) => lane.content.kind === "audio_volume",
				)}
			>
				Add audio lane
			</Button>
			<SelectField
				label="Speed Group"
				value={speedGroup}
				onChange={setSpeedGroup}
				options={["A", "B", "C", "D", "E"].map((group) => ({
					value: group,
					label: group,
				}))}
			/>
			<Button onClick={props.addSpeedLane}>Add speed lane</Button>
			<SelectField
				label="Cuelist"
				value={cueListId}
				disabled={!cueLists.length}
				onChange={setCueListId}
				options={cueLists.map((cueList) => ({
					value: cueList.id,
					label: cueList.name,
				}))}
			/>
			<Button onClick={props.addCueListLane} disabled={!cueListId}>
				Add Cuelist lane
			</Button>
			<Button onClick={() => props.setCsvOpen((open) => !open)}>
				Import marker CSV
			</Button>
			<Button disabled={!selection} onClick={copy}>
				Copy
			</Button>
			<Button disabled={!selection} onClick={remove}>
				Delete
			</Button>
			<HorizontalFader
				className="timecode-timeline-zoom"
				label="Timeline zoom"
				minimum={0.25}
				maximum={4}
				step={0.25}
				value={zoom}
				display={`${zoom}×`}
				onChange={setZoom}
			/>
		</div>
	);
}

function CsvPanel(props: {
	csvSource: string;
	setCsvSource(value: string): void;
	csvMode: "append" | "replace";
	setCsvMode(value: "append" | "replace"): void;
	csvError: string | null;
	importCsv(): void;
}) {
	return (
		<div className="timecode-csv-panel">
			<TextAreaField
				label="Marker CSV"
				value={props.csvSource}
				onChange={(event) => props.setCsvSource(event.currentTarget.value)}
			/>
			<SelectField
				label="Import mode"
				value={props.csvMode}
				onChange={props.setCsvMode}
				options={[
					{ value: "append", label: "Append" },
					{ value: "replace", label: "Replace" },
				]}
			/>
			<Button onClick={props.importCsv}>Apply marker CSV</Button>
			{props.csvError && <p role="alert">{props.csvError}</p>}
		</div>
	);
}

type TimelineItem = ReturnType<typeof timelineItems>[number];

function TimelineCanvas(props: {
	definition: TimecodeDefinition;
	frame: number;
	fps: number;
	cueLists: readonly TimecodeCueListOption[];
	waveformPeaks?: readonly number[];
	duration: number;
	width: number;
	pixelsPerFrame: number;
	items: readonly TimelineItem[];
	selection: TimecodeEditorSelection | null;
	onScrub(frame: number): void;
	startDrag(
		event: ReactPointerEvent,
		selection: TimecodeEditorSelection,
		frame: number,
	): void;
	addKeyframe(laneId: string): void;
	addClip(laneId: string): void;
}) {
	const scrub = (event: ReactPointerEvent<HTMLDivElement>) => {
		if (event.target !== event.currentTarget) return;
		const bounds = event.currentTarget.getBoundingClientRect();
		props.onScrub(
			Math.max(
				0,
				Math.min(
					props.duration,
					Math.round((event.clientX - bounds.left) / props.pixelsPerFrame),
				),
			),
		);
	};
	return (
		<div className="timecode-timeline-scroll">
			<div
				className="timecode-timeline-canvas"
				style={{ width: props.width }}
				onPointerDown={scrub}
			>
				<Ruler
					duration={props.duration}
					fps={props.fps}
					pixelsPerFrame={props.pixelsPerFrame}
				/>
				<AudioLane
					definition={props.definition}
					waveformPeaks={props.waveformPeaks}
				/>
				<div className="timecode-marker-lane">
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
								startDrag={props.startDrag}
							/>
						))}
				</div>
				{props.definition.lanes.map((lane) => (
					<EditorLane key={lane.id} {...props} lane={lane} />
				))}
				<div
					className="timecode-editor-playhead"
					style={{ left: props.frame * props.pixelsPerFrame }}
				>
					<span>{formatFrame(props.frame, props.fps)}</span>
				</div>
			</div>
		</div>
	);
}

function AudioLane({
	definition,
	waveformPeaks,
}: {
	definition: TimecodeDefinition;
	waveformPeaks?: readonly number[];
}) {
	return (
		<div
			className="timecode-audio-lane"
			role="img"
			aria-label={
				definition.audio ? "Linked audio waveform" : "No linked audio"
			}
		>
			{definition.audio ? (
				waveformPeaks?.length ? (
					<Waveform peaks={waveformPeaks} />
				) : (
					<span>Waveform loads after the managed audio is saved.</span>
				)
			) : (
				<span>No audio linked</span>
			)}
		</div>
	);
}

function EditorLane(
	props: Parameters<typeof TimelineCanvas>[0] & {
		lane: TimecodeDefinition["lanes"][number];
	},
) {
	const { lane } = props;
	const cueList = lane.content.kind === "cue_list" ? lane.content : null;
	return (
		<div className={`timecode-editor-lane lane-${lane.content.kind}`}>
			<div className="timecode-editor-lane-label">
				<strong>{lane.name}</strong>
				<span>{lane.content.kind.replaceAll("_", " ")}</span>
				{cueList ? (
					<Button
						size="compact"
						disabled={
							!props.cueLists.find(
								(candidate) => candidate.id === cueList.cue_list_id,
							)?.cues.length
						}
						onClick={() => props.addClip(lane.id)}
					>
						+ clip
					</Button>
				) : (
					<Button size="compact" onClick={() => props.addKeyframe(lane.id)}>
						+ keyframe
					</Button>
				)}
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
	): void;
}) {
	const width = item.endFrame
		? Math.max(44, (item.endFrame - item.frame) * pixelsPerFrame)
		: undefined;
	return (
		<Button
			className={`${marker ? "timecode-timeline-marker" : `timecode-timeline-item item-${item.kind}`} ${sameSelection(selection, item.selection) ? "selected" : ""}`}
			style={{ left: item.frame * pixelsPerFrame, ...(width ? { width } : {}) }}
			onPointerDown={(event) => startDrag(event, item.selection, item.frame)}
			title={`${item.label} · ${formatFrame(item.frame, fps)}`}
		>
			{item.label}
			{!marker && <small>{formatFrame(item.frame, fps)}</small>}
		</Button>
	);
}

export interface TimecodeCueListOption {
	id: string;
	name: string;
	cues: readonly { id: string; number: number; name: string }[];
}

const BASE_PIXELS_PER_SECOND = 84;

export function TimecodeTimelineEditor({
	definition,
	frame,
	fps,
	cueLists,
	waveformPeaks,
	onScrub,
	onCommit,
	onPreview,
	onBeginGesture,
	onEndGesture,
}: Props) {
	const [zoom, setZoom] = useState(1);
	const [selection, setSelection] = useState<TimecodeEditorSelection | null>(
		null,
	);
	const [csvOpen, setCsvOpen] = useState(false);
	const [csvSource, setCsvSource] = useState(
		"position,name,color\n00:00:05:00,Intro,#a67cff",
	);
	const [csvMode, setCsvMode] = useState<"append" | "replace">("append");
	const [csvError, setCsvError] = useState<string | null>(null);
	const [speedGroup, setSpeedGroup] = useState("A");
	const [cueListId, setCueListId] = useState(cueLists[0]?.id ?? "");
	const duration = Math.max(1, definition.duration_frame ?? fps * 60);
	const pixelsPerFrame = (BASE_PIXELS_PER_SECOND * zoom) / fps;
	const width = Math.max(720, duration * pixelsPerFrame);
	const items = useMemo(() => timelineItems(definition), [definition]);
	const selected = items.find((item) =>
		sameSelection(item.selection, selection),
	);

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
		addAudioLane,
		addSpeedLane,
		addCueListLane,
		addClip,
		importCsv,
	} = useTimelineActions({
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
	});

	return (
		<section
			className="timecode-timeline-editor"
			aria-label="Timecode timeline editor"
		>
			<TimelineTools
				{...{
					definition,
					selection,
					speedGroup,
					setSpeedGroup,
					cueListId,
					setCueListId,
					cueLists,
					zoom,
					setZoom,
					addMarker,
					addAudioLane,
					addSpeedLane,
					addCueListLane,
					setCsvOpen,
					setSelection,
					onCommit,
					fps,
				}}
			/>
			{csvOpen && (
				<CsvPanel
					{...{
						csvSource,
						setCsvSource,
						csvMode,
						setCsvMode,
						csvError,
						importCsv,
					}}
				/>
			)}
			<TimelineCanvas
				{...{
					definition,
					frame,
					fps,
					cueLists,
					waveformPeaks,
					duration,
					width,
					pixelsPerFrame,
					items,
					selection,
					onScrub,
					startDrag,
					addKeyframe,
					addClip,
				}}
			/>
			<SelectionInspector
				definition={definition}
				selection={selection}
				selectedLabel={selected?.label}
				cueLists={cueLists}
				fps={fps}
				onCommit={onCommit}
			/>
		</section>
	);
}

function SelectionInspector({
	definition,
	selection,
	selectedLabel,
	cueLists,
	fps,
	onCommit,
}: {
	definition: TimecodeDefinition;
	selection: TimecodeEditorSelection | null;
	selectedLabel?: string;
	cueLists: readonly TimecodeCueListOption[];
	fps: number;
	onCommit(definition: TimecodeDefinition): void;
}) {
	if (!selection)
		return (
			<div className="timecode-selection-inspector">
				<span>
					Select a clip, keyframe, or marker to inspect, copy, move, or delete
					it.
				</span>
			</div>
		);
	if (selection.kind === "marker") {
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
	return null;
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
	cues: readonly { id: string; number: number; name: string }[];
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
	cues: readonly { id: string; number: number; name: string }[];
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
		<div className="timecode-ruler">
			{ticks.map((tick) => (
				<span key={tick} style={{ left: tick * pixelsPerFrame }}>
					{formatFrame(tick, fps)}
				</span>
			))}
		</div>
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
	return `${String(Math.floor(seconds / 3600)).padStart(2, "0")}:${String(Math.floor(seconds / 60) % 60).padStart(2, "0")}:${String(seconds % 60).padStart(2, "0")}:${String(whole % fps).padStart(2, "0")}`;
}
