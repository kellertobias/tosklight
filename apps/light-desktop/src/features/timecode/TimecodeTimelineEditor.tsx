import { Button, Input } from "@tosklight/ui";
import {
	type PointerEvent as ReactPointerEvent,
	useEffect,
	useId,
	useMemo,
	useRef,
	useState,
} from "react";
import type { TimecodeDefinition } from "../../api/generated/light-wire";
import {
	copyTimelineItem,
	deleteTimelineItem,
	moveTimelineItem,
	parseMarkerCsv,
	sameSelection,
	snapTimelineFrame,
	type TimecodeEditorSelection,
	timelineItems,
} from "./editorModel";

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

export interface TimecodeCueListOption {
	id: string;
	name: string;
	cues: readonly { id: string; number: number; name: string }[];
}

interface DragState {
	selection: TimecodeEditorSelection;
	startX: number;
	startFrame: number;
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
	const drag = useRef<DragState | null>(null);
	const latest = useRef(definition);
	latest.current = definition;
	const duration = Math.max(1, definition.duration_frame ?? fps * 60);
	const pixelsPerFrame = (BASE_PIXELS_PER_SECOND * zoom) / fps;
	const width = Math.max(720, duration * pixelsPerFrame);
	const items = useMemo(() => timelineItems(definition), [definition]);
	const selected = items.find((item) =>
		sameSelection(item.selection, selection),
	);

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
			if (!drag.current) return;
			drag.current = null;
			onEndGesture();
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

	const startDrag = (
		event: ReactPointerEvent,
		itemSelection: TimecodeEditorSelection,
		startFrame: number,
	) => {
		event.preventDefault();
		setSelection(itemSelection);
		onBeginGesture();
		drag.current = {
			selection: itemSelection,
			startX: event.clientX,
			startFrame,
		};
		event.currentTarget.setPointerCapture?.(event.pointerId);
	};

	const addMarker = () => {
		const id = crypto.randomUUID();
		onCommit({
			...definition,
			markers: [
				...definition.markers,
				{
					id,
					frame: Math.min(frame, duration),
					name: `Marker ${definition.markers.length + 1}`,
				},
			],
		});
		setSelection({ kind: "marker", itemId: id });
	};

	const addKeyframe = (laneId: string) => {
		const id = crypto.randomUUID();
		onCommit({
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
								{
									id,
									frame,
									value: 1,
									fade_frames: 0,
									curve: "linear" as const,
								},
							],
						},
					};
				return lane;
			}),
		});
		const lane = definition.lanes.find((candidate) => candidate.id === laneId);
		if (lane?.content.kind === "speed_group")
			setSelection({ kind: "speed", laneId, itemId: id });
		if (lane?.content.kind === "audio_volume")
			setSelection({ kind: "volume", laneId, itemId: id });
	};

	const addAudioLane = () => {
		if (definition.lanes.some((lane) => lane.content.kind === "audio_volume"))
			return;
		onCommit({
			...definition,
			lanes: [
				...definition.lanes,
				{
					id: crypto.randomUUID(),
					name: "Main audio volume",
					content: { kind: "audio_volume", keyframes: [] },
				},
			],
		});
	};

	const addSpeedLane = () => {
		onCommit({
			...definition,
			lanes: [
				...definition.lanes,
				{
					id: crypto.randomUUID(),
					name: `Speed Group ${speedGroup}`,
					content: { kind: "speed_group", group: speedGroup, keyframes: [] },
				},
			],
		});
	};

	const addCueListLane = () => {
		const cueList = cueLists.find((candidate) => candidate.id === cueListId);
		if (!cueList) return;
		onCommit({
			...definition,
			lanes: [
				...definition.lanes,
				{
					id: crypto.randomUUID(),
					name: cueList.name,
					content: {
						kind: "cue_list",
						cue_list_id: cueList.id,
						clips: [],
					},
				},
			],
		});
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
		const end = Math.min(duration, start + fps * 4);
		onCommit({
			...definition,
			lanes: definition.lanes.map((candidate) =>
				candidate.id !== laneId || candidate.content.kind !== "cue_list"
					? candidate
					: {
							...candidate,
							content: {
								...candidate.content,
								clips: [
									...candidate.content.clips,
									{
										id,
										start_frame: start,
										end_frame: end,
										start_cue_id: first.id,
										end_cue_id: last.id,
										start_behavior: "state" as const,
										end_behavior: "release" as const,
									},
								],
							},
						},
			),
		});
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

	return (
		<section
			className="timecode-timeline-editor"
			aria-label="Timecode timeline editor"
		>
			<div className="timecode-timeline-tools">
				<Button onClick={addMarker}>Add marker at playhead</Button>
				<Button
					onClick={addAudioLane}
					disabled={definition.lanes.some(
						(lane) => lane.content.kind === "audio_volume",
					)}
				>
					Add audio lane
				</Button>
				<label htmlFor="timecode-new-speed-group">
					Speed Group
					<select
						id="timecode-new-speed-group"
						value={speedGroup}
						onChange={(event) => setSpeedGroup(event.currentTarget.value)}
					>
						{["A", "B", "C", "D", "E"].map((group) => (
							<option key={group}>{group}</option>
						))}
					</select>
				</label>
				<Button onClick={addSpeedLane}>Add speed lane</Button>
				<label htmlFor="timecode-new-cue-list">
					Cuelist
					<select
						id="timecode-new-cue-list"
						value={cueListId}
						disabled={!cueLists.length}
						onChange={(event) => setCueListId(event.currentTarget.value)}
					>
						{cueLists.map((cueList) => (
							<option key={cueList.id} value={cueList.id}>
								{cueList.name}
							</option>
						))}
					</select>
				</label>
				<Button onClick={addCueListLane} disabled={!cueListId}>
					Add Cuelist lane
				</Button>
				<Button onClick={() => setCsvOpen((open) => !open)}>
					Import marker CSV
				</Button>
				<Button
					disabled={!selection}
					onClick={() => {
						if (!selection) return;
						const copied = copyTimelineItem(
							definition,
							selection,
							crypto.randomUUID(),
							fps,
						);
						onCommit(copied.definition);
						setSelection(copied.selection);
					}}
				>
					Copy
				</Button>
				<Button
					disabled={!selection}
					onClick={() => {
						if (!selection) return;
						onCommit(deleteTimelineItem(definition, selection));
						setSelection(null);
					}}
				>
					Delete
				</Button>
				<label htmlFor="timecode-timeline-zoom">
					Zoom
					<Input
						id="timecode-timeline-zoom"
						aria-label="Timeline zoom"
						type="range"
						min="0.25"
						max="4"
						step="0.25"
						value={zoom}
						onChange={(event) => setZoom(Number(event.currentTarget.value))}
					/>
				</label>
			</div>
			{csvOpen && (
				<div className="timecode-csv-panel">
					<label>
						Marker CSV
						<textarea
							aria-label="Marker CSV"
							value={csvSource}
							onChange={(event) => setCsvSource(event.currentTarget.value)}
						/>
					</label>
					<label>
						Import mode
						<select
							value={csvMode}
							onChange={(event) =>
								setCsvMode(event.currentTarget.value as "append" | "replace")
							}
						>
							<option value="append">Append</option>
							<option value="replace">Replace</option>
						</select>
					</label>
					<Button onClick={importCsv}>Apply marker CSV</Button>
					{csvError && <p role="alert">{csvError}</p>}
				</div>
			)}
			<div className="timecode-timeline-scroll">
				<div
					className="timecode-timeline-canvas"
					style={{ width }}
					onPointerDown={(event) => {
						if (event.target !== event.currentTarget) return;
						const bounds = event.currentTarget.getBoundingClientRect();
						onScrub(
							Math.max(
								0,
								Math.min(
									duration,
									Math.round((event.clientX - bounds.left) / pixelsPerFrame),
								),
							),
						);
					}}
				>
					<Ruler
						duration={duration}
						fps={fps}
						pixelsPerFrame={pixelsPerFrame}
					/>
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
					<div className="timecode-marker-lane">
						{items
							.filter((item) => item.kind === "marker")
							.map((item) => (
								<button
									key={item.selection.itemId}
									type="button"
									className={`timecode-timeline-marker ${sameSelection(selection, item.selection) ? "selected" : ""}`}
									style={{ left: item.frame * pixelsPerFrame }}
									onPointerDown={(event) =>
										startDrag(event, item.selection, item.frame)
									}
									title={`${item.label} · ${formatFrame(item.frame, fps)}`}
								>
									{item.label}
								</button>
							))}
					</div>
					{definition.lanes.map((lane) => {
						const laneItems = items.filter((item) => item.laneId === lane.id);
						const cueListContent =
							lane.content.kind === "cue_list" ? lane.content : null;
						return (
							<div
								className={`timecode-editor-lane lane-${lane.content.kind}`}
								key={lane.id}
							>
								<div className="timecode-editor-lane-label">
									<strong>{lane.name}</strong>
									<span>{lane.content.kind.replaceAll("_", " ")}</span>
									{!cueListContent ? (
										<Button size="compact" onClick={() => addKeyframe(lane.id)}>
											+ keyframe
										</Button>
									) : (
										<Button
											size="compact"
											disabled={
												!cueLists.find(
													(candidate) =>
														candidate.id === cueListContent.cue_list_id,
												)?.cues.length
											}
											onClick={() => addClip(lane.id)}
										>
											+ clip
										</Button>
									)}
								</div>
								{laneItems.map((item) => (
									<button
										key={item.selection.itemId}
										type="button"
										className={`timecode-timeline-item item-${item.kind} ${sameSelection(selection, item.selection) ? "selected" : ""}`}
										style={{
											left: item.frame * pixelsPerFrame,
											...(item.endFrame
												? {
														width: Math.max(
															44,
															(item.endFrame - item.frame) * pixelsPerFrame,
														),
													}
												: {}),
										}}
										onPointerDown={(event) =>
											startDrag(event, item.selection, item.frame)
										}
										title={`${item.label} · ${formatFrame(item.frame, fps)}`}
									>
										{item.label}
										<small>{formatFrame(item.frame, fps)}</small>
									</button>
								))}
							</div>
						);
					})}
					<div
						className="timecode-editor-playhead"
						style={{ left: frame * pixelsPerFrame }}
					>
						<span>{formatFrame(frame, fps)}</span>
					</div>
				</div>
			</div>
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
			<div className="timecode-selection-inspector">
				<strong>{selectedLabel}</strong>
				<label htmlFor={`timecode-marker-name-${marker.id}`}>
					Name
					<Input
						id={`timecode-marker-name-${marker.id}`}
						value={marker.name}
						onChange={(event) =>
							onCommit({
								...definition,
								markers: definition.markers.map((candidate) =>
									candidate.id === marker.id
										? { ...candidate, name: event.currentTarget.value }
										: candidate,
								),
							})
						}
					/>
				</label>
				<label htmlFor={`timecode-marker-color-${marker.id}`}>
					Color
					<Input
						id={`timecode-marker-color-${marker.id}`}
						type="color"
						value={marker.color ?? "#a67cff"}
						onChange={(event) =>
							onCommit({
								...definition,
								markers: definition.markers.map((candidate) =>
									candidate.id === marker.id
										? { ...candidate, color: event.currentTarget.value }
										: candidate,
								),
							})
						}
					/>
				</label>
				<span>Trigger {formatFrame(marker.frame, fps)}</span>
			</div>
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
			<div className="timecode-selection-inspector">
				<strong>{selectedLabel}</strong>
				<InspectorNumber
					label="BPM"
					value={keyframe.bpm}
					min={1}
					max={999}
					onValue={(bpm) =>
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
				/>
				<InspectorNumber
					label="Phase"
					value={keyframe.phase}
					min={0}
					max={1}
					step={0.01}
					onValue={(phase) =>
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
				<span>Trigger {formatFrame(keyframe.frame, fps)}</span>
			</div>
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
			<div className="timecode-selection-inspector">
				<strong>{selectedLabel}</strong>
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
				<label>
					Curve
					<select
						value={keyframe.curve}
						onChange={(event) =>
							update({
								curve: event.currentTarget.value as typeof keyframe.curve,
							})
						}
					>
						<option value="linear">Linear</option>
						<option value="ease_in">Ease in</option>
						<option value="ease_out">Ease out</option>
						<option value="ease_in_out">Ease in/out</option>
					</select>
				</label>
				<span>Trigger {formatFrame(keyframe.frame, fps)}</span>
			</div>
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
			<div className="timecode-selection-inspector">
				<strong>{selectedLabel}</strong>
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
					max={definition.duration_frame ?? undefined}
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
				<label>
					Start behavior
					<select
						value={clip.start_behavior}
						onChange={(event) =>
							update({
								start_behavior: event.currentTarget
									.value as typeof clip.start_behavior,
							})
						}
					>
						<option value="state">State Start</option>
						<option value="cue">Cue Start</option>
					</select>
				</label>
				<label>
					End behavior
					<select
						value={clip.end_behavior}
						onChange={(event) =>
							update({
								end_behavior: event.currentTarget
									.value as typeof clip.end_behavior,
							})
						}
					>
						<option value="release">Release</option>
						<option value="hold">Hold</option>
					</select>
				</label>
			</div>
		);
	}
	return null;
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
	const id = useId();
	return (
		<label htmlFor={id}>
			{label}
			<Input
				id={id}
				type="number"
				value={value}
				min={min}
				max={max}
				step={step}
				onChange={(event) => onValue(Number(event.currentTarget.value))}
			/>
		</label>
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
		<label>
			{label}
			<select
				value={value}
				onChange={(event) => onValue(event.currentTarget.value)}
			>
				{cues.map((cue) => (
					<option key={cue.id} value={cue.id}>
						{cue.number} · {cue.name}
					</option>
				))}
			</select>
		</label>
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
