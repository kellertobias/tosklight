import { Button, Input } from "@tosklight/ui";
import {
	type PointerEvent as ReactPointerEvent,
	useEffect,
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
	waveformPeaks?: readonly number[];
	onScrub(frame: number): void;
	onCommit(definition: TimecodeDefinition): void;
	onPreview(definition: TimecodeDefinition): void;
	onBeginGesture(): void;
	onEndGesture(): void;
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
						return (
							<div
								className={`timecode-editor-lane lane-${lane.content.kind}`}
								key={lane.id}
							>
								<div className="timecode-editor-lane-label">
									<strong>{lane.name}</strong>
									<span>{lane.content.kind.replaceAll("_", " ")}</span>
									{lane.content.kind !== "cue_list" && (
										<Button size="compact" onClick={() => addKeyframe(lane.id)}>
											+ keyframe
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
			<div className="timecode-selection-inspector" aria-live="polite">
				{selected ? (
					<>
						<strong>{selected.label}</strong>
						<span>
							Trigger {formatFrame(selected.frame, fps)} · drag to move; markers
							snap within 12 px
						</span>
					</>
				) : (
					<span>
						Select a clip, keyframe, or marker to inspect, copy, move, or delete
						it.
					</span>
				)}
			</div>
		</section>
	);
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
