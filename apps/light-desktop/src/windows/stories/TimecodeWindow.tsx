/** Historical full-desk composition retained for visual comparison with the production editor. */
import { Button, Input, WindowHeader } from "@tosklight/ui";
import {
	type CSSProperties,
	type PointerEvent as ReactPointerEvent,
	useEffect,
	useMemo,
	useRef,
	useState,
} from "react";
import "../TimecodeWindow.css";

export const TIMECODE_HZ = 44;
export const TIMECODE_DURATION_SECONDS = 75;
export const TIMECODE_TOTAL_FRAMES = TIMECODE_HZ * TIMECODE_DURATION_SECONDS;

export type TimecodeLaneKind = "cuelist" | "group-master" | "speed-group";

export interface TimecodeCue {
	id: string;
	number: string;
	name: string;
	go: number;
	fade: number;
}

export interface TimecodeCuelistInstance {
	id: string;
	start: number;
	end: number;
	cues: TimecodeCue[];
}

export interface TimecodeAutomationPoint {
	id: string;
	frame: number;
	value: number;
	fade: number;
	restart?: boolean;
}

export interface TimecodeLane {
	id: string;
	kind: TimecodeLaneKind;
	objectId: string;
	label: string;
	color: string;
	instances?: TimecodeCuelistInstance[];
	points?: TimecodeAutomationPoint[];
}

export interface TimecodeSelection {
	laneId: string;
	instanceId?: string;
	pointId: string;
}

export interface TimecodeWindowProps {
	lanes: TimecodeLane[];
	selection: TimecodeSelection | null;
	frame: number;
	playing: boolean;
	rate: number;
	zoom: number;
	verticalZoom: number;
	loopStart: number;
	loopEnd: number;
	audioFile: string | null;
	onLanes(lanes: TimecodeLane[]): void;
	onSelection(selection: TimecodeSelection): void;
	onFrame(frame: number): void;
	onPlaying(playing: boolean): void;
	onRate(rate: number): void;
	onZoom(zoom: number): void;
	onVerticalZoom(zoom: number): void;
	onAudioFile(file: string | null): void;
}

type DragTarget =
	| {
			kind: "playhead";
			startX: number;
			startValue: number;
	  }
	| {
			kind: "instance" | "instance-end";
			startX: number;
			startValue: number;
			laneId: string;
			instanceId: string;
	  }
	| {
			kind: "cue-go" | "cue-fade";
			startX: number;
			startValue: number;
			laneId: string;
			instanceId: string;
			pointId: string;
	  }
	| {
			kind: "automation" | "automation-fade";
			startX: number;
			startValue: number;
			laneId: string;
			pointId: string;
	  };
type DragStart = DragTarget extends infer Target
	? Target extends { startX: number }
		? Omit<Target, "startX">
		: never
	: never;

function clampFrame(frame: number) {
	return Math.max(0, Math.min(TIMECODE_TOTAL_FRAMES, Math.round(frame)));
}

export function secondsToFrames(seconds: number) {
	return Math.round(seconds * TIMECODE_HZ);
}

export function formatTimecode(frame: number) {
	const safe = clampFrame(frame);
	const subsecond = safe % TIMECODE_HZ;
	const totalSeconds = Math.floor(safe / TIMECODE_HZ);
	const seconds = totalSeconds % 60;
	const minutes = Math.floor(totalSeconds / 60);
	return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}:${String(subsecond).padStart(2, "0")}`;
}

function laneKindLabel(kind: TimecodeLaneKind) {
	if (kind === "cuelist") return "Cuelist";
	if (kind === "group-master") return "Group Master";
	return "Speed Group";
}

function cueSelection(
	lane: TimecodeLane,
	instance: TimecodeCuelistInstance,
	cue: TimecodeCue,
): TimecodeSelection {
	return { laneId: lane.id, instanceId: instance.id, pointId: cue.id };
}

function automationSelection(
	lane: TimecodeLane,
	point: TimecodeAutomationPoint,
): TimecodeSelection {
	return { laneId: lane.id, pointId: point.id };
}

function firstSelection(lanes: TimecodeLane[]) {
	for (const lane of lanes) {
		const instance = lane.instances?.[0];
		const cue = instance?.cues[0];
		if (instance && cue) return cueSelection(lane, instance, cue);
		const point = lane.points?.[0];
		if (point) return automationSelection(lane, point);
	}
	return null;
}

function TransportButton({
	children,
	label,
	active,
	onClick,
}: {
	children: React.ReactNode;
	label: string;
	active?: boolean;
	onClick(): void;
}) {
	return (
		<Button
			size="compact"
			active={active}
			aria-label={label}
			title={label}
			onClick={onClick}
		>
			{children}
		</Button>
	);
}

function Waveform({ width }: { width: number }) {
	const bars = useMemo(
		() =>
			Array.from({ length: 220 }, (_, index) => {
				const envelope = Math.sin((index / 219) * Math.PI);
				const texture =
					0.28 +
					Math.abs(Math.sin(index * 0.59)) * 0.44 +
					Math.abs(Math.cos(index * 0.17)) * 0.28;
				return Math.max(2, Math.round(34 * envelope * texture));
			}),
		[],
	);
	return (
		<svg
			className="timecode-waveform"
			viewBox={`0 0 ${width} 44`}
			preserveAspectRatio="none"
			aria-hidden="true"
		>
			{bars.map((height, index) => {
				const x = (index / bars.length) * width;
				return (
					<line
						key={`${index}-${height}`}
						x1={x}
						x2={x}
						y1={22 - height / 2}
						y2={22 + height / 2}
					/>
				);
			})}
		</svg>
	);
}

export function TimecodeWindow({
	lanes,
	selection,
	frame,
	playing,
	rate,
	zoom,
	verticalZoom,
	loopStart,
	loopEnd,
	audioFile,
	onLanes,
	onSelection,
	onFrame,
	onPlaying,
	onRate,
	onZoom,
	onVerticalZoom,
	onAudioFile,
}: TimecodeWindowProps) {
	const [laneMenu, setLaneMenu] = useState(false);
	const drag = useRef<DragTarget | null>(null);
	const scroll = useRef<HTMLElement | null>(null);
	const audioInput = useRef<HTMLInputElement | null>(null);
	const initialScrollComplete = useRef(false);
	const latestFrame = useRef(frame);
	latestFrame.current = frame;
	const pixelsPerSecond = 86 * zoom;
	const pixelsPerFrame = pixelsPerSecond / TIMECODE_HZ;
	const timelineWidth = TIMECODE_DURATION_SECONDS * pixelsPerSecond;
	const framePx = (value: number) => (value / TIMECODE_HZ) * pixelsPerSecond;
	const framesFromPx = (value: number) => value / pixelsPerFrame;

	useEffect(() => {
		if (initialScrollComplete.current || !scroll.current) return;
		scroll.current.scrollLeft = Math.max(0, framePx(frame) - 360);
		initialScrollComplete.current = true;
	}, [frame, pixelsPerSecond]);

	useEffect(() => {
		if (!playing) return;
		let request = 0;
		let previous = performance.now();
		const tick = (now: number) => {
			const elapsed = now - previous;
			previous = now;
			const next = latestFrame.current + (elapsed / 1000) * TIMECODE_HZ * rate;
			if (rate > 0 && next >= loopEnd) {
				onFrame(loopStart);
				request = window.requestAnimationFrame(tick);
				return;
			}
			if (rate < 0 && next <= loopStart) {
				onFrame(loopEnd);
				request = window.requestAnimationFrame(tick);
				return;
			}
			if (next <= 0 || next >= TIMECODE_TOTAL_FRAMES) {
				onFrame(next <= 0 ? 0 : TIMECODE_TOTAL_FRAMES);
				onPlaying(false);
				return;
			}
			onFrame(next);
			request = window.requestAnimationFrame(tick);
		};
		request = window.requestAnimationFrame(tick);
		return () => window.cancelAnimationFrame(request);
	}, [loopEnd, loopStart, onFrame, onPlaying, playing, rate]);

	useEffect(() => {
		const move = (event: PointerEvent) => {
			const active = drag.current;
			if (!active) return;
			const delta = framesFromPx(event.clientX - active.startX);
			if (active.kind === "playhead") {
				onFrame(clampFrame(active.startValue + delta));
				return;
			}
			onLanes(
				lanes.map((lane) => {
					if (lane.id !== active.laneId) return lane;
					if (active.kind === "instance" || active.kind === "instance-end") {
						return {
							...lane,
							instances: lane.instances?.map((instance) => {
								if (instance.id !== active.instanceId) return instance;
								if (active.kind === "instance-end")
									return {
										...instance,
										end: Math.min(
											TIMECODE_TOTAL_FRAMES,
											Math.max(
												instance.start +
													Math.max(
														TIMECODE_HZ,
														...instance.cues.map((cue) => cue.go + cue.fade),
													),
												clampFrame(active.startValue + delta),
											),
										),
									};
								const duration = instance.end - instance.start;
								const start = Math.min(
									TIMECODE_TOTAL_FRAMES - duration,
									clampFrame(active.startValue + delta),
								);
								return { ...instance, start, end: start + duration };
							}),
						};
					}
					if (active.kind === "cue-go" || active.kind === "cue-fade") {
						return {
							...lane,
							instances: lane.instances?.map((instance) =>
								instance.id !== active.instanceId
									? instance
									: {
											...instance,
											cues: instance.cues.map((cue) =>
												cue.id !== active.pointId
													? cue
													: active.kind === "cue-go"
														? {
																...cue,
																go: Math.max(
																	0,
																	Math.min(
																		instance.end - instance.start,
																		Math.round(active.startValue + delta),
																	),
																),
															}
														: {
																...cue,
																fade: Math.max(
																	0,
																	Math.round(active.startValue + delta),
																),
															},
											),
										},
							),
						};
					}
					if (active.kind !== "automation" && active.kind !== "automation-fade")
						return lane;
					return {
						...lane,
						points: lane.points?.map((point) =>
							point.id !== active.pointId
								? point
								: active.kind === "automation"
									? {
											...point,
											frame: clampFrame(active.startValue + delta),
										}
									: {
											...point,
											fade: Math.max(0, Math.round(active.startValue + delta)),
										},
						),
					};
				}),
			);
		};
		const up = () => {
			drag.current = null;
		};
		window.addEventListener("pointermove", move);
		window.addEventListener("pointerup", up);
		window.addEventListener("pointercancel", up);
		return () => {
			window.removeEventListener("pointermove", move);
			window.removeEventListener("pointerup", up);
			window.removeEventListener("pointercancel", up);
		};
	}, [framesFromPx, lanes, onFrame, onLanes]);

	const begin = (event: ReactPointerEvent, target: DragStart) => {
		event.preventDefault();
		event.stopPropagation();
		drag.current = { ...target, startX: event.clientX } as DragTarget;
	};

	const repeatCuelist = (lane: TimecodeLane) => {
		const source = lane.instances?.at(-1);
		if (!source) {
			const duration = secondsToFrames(10);
			const start = Math.min(
				TIMECODE_TOTAL_FRAMES - duration,
				clampFrame(frame),
			);
			const suffix = Date.now();
			onLanes(
				lanes.map((item) =>
					item.id === lane.id
						? {
								...item,
								instances: [
									{
										id: `instance-${suffix}`,
										start,
										end: start + duration,
										cues: [
											{
												id: `cue-${suffix}`,
												number: "1",
												name: "First cue",
												go: 0,
												fade: secondsToFrames(2),
											},
										],
									},
								],
							}
						: item,
				),
			);
			return;
		}
		const duration = source.end - source.start;
		const start = Math.min(
			TIMECODE_TOTAL_FRAMES - duration,
			source.end + secondsToFrames(2),
		);
		const suffix = Date.now();
		onLanes(
			lanes.map((item) =>
				item.id !== lane.id
					? item
					: {
							...item,
							instances: [
								...(item.instances ?? []),
								{
									...source,
									id: `${source.id}-repeat-${suffix}`,
									start,
									end: start + duration,
									cues: source.cues.map((cue) => ({
										...cue,
										id: `${cue.id}-repeat-${suffix}`,
									})),
								},
							],
						},
			),
		);
	};

	const removeLaneContent = (lane: TimecodeLane) => {
		if (lane.kind === "cuelist" && (lane.instances?.length ?? 0) > 0) {
			const removed = lane.instances?.at(-1);
			const next = lanes.map((item) =>
				item.id === lane.id
					? { ...item, instances: item.instances?.slice(0, -1) }
					: item,
			);
			onLanes(next);
			if (selection?.instanceId === removed?.id) {
				const fallback = firstSelection(next);
				if (fallback) onSelection(fallback);
			}
			return;
		}
		const next = lanes.filter((item) => item.id !== lane.id);
		onLanes(next);
		if (selection?.laneId === lane.id) {
			const fallback = firstSelection(next);
			if (fallback) onSelection(fallback);
		}
	};

	const addLane = (
		kind: TimecodeLaneKind,
		objectId: string,
		label: string,
		color: string,
	) => {
		const suffix = Date.now();
		onLanes([
			...lanes,
			kind === "cuelist"
				? {
						id: `lane-${suffix}`,
						kind,
						objectId,
						label,
						color,
						instances: [],
					}
				: {
						id: `lane-${suffix}`,
						kind,
						objectId,
						label,
						color,
						points: [
							{
								id: `point-${suffix}`,
								frame: clampFrame(frame),
								value: kind === "group-master" ? 100 : 120,
								fade: kind === "group-master" ? TIMECODE_HZ : 0,
								restart: kind === "speed-group",
							},
						],
					},
		]);
		setLaneMenu(false);
	};

	const ruler = useMemo(
		() =>
			Array.from(
				{ length: TIMECODE_DURATION_SECONDS + 1 },
				(_, second) => second,
			),
		[],
	);

	return (
		<section
			className="timecode-window"
			style={
				{
					"--timecode-lane-height": `${Math.round(82 * verticalZoom)}px`,
				} as CSSProperties
			}
		>
			<WindowHeader
				title="Timecode"
				info={{
					primary: "Timecode 1 · Show Opener",
					secondary: `${audioFile ?? "No song"} · ${TIMECODE_HZ} Hz · ${lanes.length} lanes · H ${Math.round(zoom * 100)}% · V ${Math.round(verticalZoom * 100)}%`,
				}}
				groups={[
					{ id: "timecode-audio", actions: [
						{
							id: "change-song",
							label: audioFile ? "Change Song" : "Assign Song",
							onPress: () => audioInput.current?.click(),
						},
					] },
					{ id: "timecode-horizontal-zoom", actions: [
						{
							id: "zoom-out",
							label: "H −",
							ariaLabel: "Horizontal zoom out",
							onPress: () => onZoom(Math.max(0.55, zoom / 1.35)),
						},
						{
							id: "zoom-in",
							label: "H +",
							ariaLabel: "Horizontal zoom in",
							onPress: () => onZoom(Math.min(4.5, zoom * 1.35)),
						},
					] },
					{ id: "timecode-vertical-zoom", actions: [
						{
							id: "vertical-zoom-out",
							label: "V −",
							ariaLabel: "Vertical zoom out",
							onPress: () => onVerticalZoom(Math.max(0.7, verticalZoom / 1.25)),
						},
						{
							id: "vertical-zoom-in",
							label: "V +",
							ariaLabel: "Vertical zoom in",
							onPress: () =>
								onVerticalZoom(Math.min(1.75, verticalZoom * 1.25)),
						},
					] },
					{ id: "timecode-lanes", actions: [
						{
							id: "add-lane",
							label: "+ Lane",
							variant: "primary",
							active: laneMenu,
							onPress: () => setLaneMenu((value) => !value),
						},
					] },
				]}
			/>
			<Input
				ref={audioInput}
				className="timecode-hidden-file-input"
				type="file"
				accept="audio/*"
				onChange={(event) => onAudioFile(event.target.files?.[0]?.name ?? null)}
			/>
			{laneMenu && (
				<div className="timecode-lane-menu">
					<strong>One object type per lane</strong>
					<Button
						contentAlign="left"
						onClick={() =>
							addLane("cuelist", "cuelist-3", "Cuelist 3 · Finale", "#a9e34b")
						}
					>
						Cuelist 3 · Finale
					</Button>
					<Button
						contentAlign="left"
						onClick={() =>
							addLane("cuelist", "cuelist-1", "Cuelist 1 · Opening", "#8ed947")
						}
					>
						Cuelist 1 · Opening (new lane)
					</Button>
					<Button
						contentAlign="left"
						onClick={() =>
							addLane(
								"group-master",
								"group-1",
								"Group 1 · Front Wash",
								"#f5bd4f",
							)
						}
					>
						Group Master · Front Wash
					</Button>
					<Button
						contentAlign="left"
						onClick={() =>
							addLane(
								"speed-group",
								"speed-group-b",
								"Speed Group B",
								"#aa8bff",
							)
						}
					>
						Speed Group B
					</Button>
				</div>
			)}
			<div className="timecode-editor">
				<section
					ref={scroll}
					className="timecode-scroll"
					aria-label="Timecode timeline editor"
				>
					<div
						className="timecode-canvas"
						style={{ width: timelineWidth + 196 }}
					>
						<div className="timecode-ruler-row">
							<div className="timecode-corner">
								<strong>LANES</strong>
								<small>One object type per lane</small>
							</div>
							<div
								className="timecode-ruler"
								style={{ width: timelineWidth }}
								onPointerDown={(event) => {
									const bounds = event.currentTarget.getBoundingClientRect();
									const next = clampFrame(
										framesFromPx(event.clientX - bounds.left),
									);
									onFrame(next);
									begin(event, {
										kind: "playhead",
										startValue: next,
									});
								}}
							>
								{ruler.map((second) => (
									<span
										key={second}
										className="timecode-ruler-second"
										style={{ left: second * pixelsPerSecond }}
									>
										<b>{formatTimecode(second * TIMECODE_HZ)}</b>
									</span>
								))}
							</div>
						</div>
						{audioFile && (
							<div className="timecode-audio-row">
								<div className="timecode-lane-label audio">
									<i />
									<div>
										<strong>{audioFile}</strong>
										<small>Audio · follows transport and scrubbing</small>
									</div>
								</div>
								<div
									className="timecode-audio-track"
									style={{ width: timelineWidth }}
								>
									<Waveform width={timelineWidth} />
								</div>
							</div>
						)}
						{lanes.map((lane) => (
							<div className="timecode-lane-row" key={lane.id}>
								<div className="timecode-lane-label">
									<i style={{ background: lane.color }} />
									<div>
										<strong>{lane.label}</strong>
										<small>
											{laneKindLabel(lane.kind)} · {lane.objectId}
										</small>
									</div>
									<div className="timecode-lane-actions">
										{lane.kind === "cuelist" && (
											<Button
												size="compact"
												aria-label={`Repeat ${lane.label}`}
												title="Repeat at end"
												onClick={() => repeatCuelist(lane)}
											>
												↻
											</Button>
										)}
										<Button
											size="compact"
											aria-label={`Remove from ${lane.label}`}
											title={
												lane.kind === "cuelist" &&
												(lane.instances?.length ?? 0) > 0
													? "Remove last Cuelist instance"
													: "Delete empty lane"
											}
											onClick={() => removeLaneContent(lane)}
										>
											×
										</Button>
									</div>
								</div>
								<section
									className={`timecode-track ${lane.kind}`}
									aria-label={`${lane.label} timeline lane`}
									style={
										{
											width: timelineWidth,
											"--second-width": `${pixelsPerSecond}px`,
											"--frame-width": `${Math.max(1, pixelsPerFrame)}px`,
										} as CSSProperties
									}
									onDoubleClick={(event) => {
										if (lane.kind === "cuelist") return;
										const bounds = event.currentTarget.getBoundingClientRect();
										const point: TimecodeAutomationPoint = {
											id: `${lane.id}-point-${Date.now()}`,
											frame: clampFrame(
												framesFromPx(event.clientX - bounds.left),
											),
											value: lane.kind === "group-master" ? 75 : 128,
											fade: lane.kind === "group-master" ? TIMECODE_HZ : 0,
											restart: lane.kind === "speed-group",
										};
										onLanes(
											lanes.map((item) =>
												item.id === lane.id
													? {
															...item,
															points: [...(item.points ?? []), point],
														}
													: item,
											),
										);
										onSelection(automationSelection(lane, point));
									}}
								>
									{lane.kind === "cuelist" &&
										(lane.instances?.length ?? 0) === 0 && (
											<Button
												type="button"
												className="timecode-empty-lane"
												onClick={() => repeatCuelist(lane)}
											>
												+ Add {lane.label} at playhead
											</Button>
										)}
									{lane.instances?.map((instance, instanceIndex) => (
										<div
											className="timecode-cuelist-instance"
											key={instance.id}
											style={
												{
													left: framePx(instance.start),
													width: framePx(instance.end - instance.start),
													"--lane-color": lane.color,
												} as CSSProperties
											}
										>
											<legend className="visually-hidden">
												{lane.label} timeline lane
											</legend>
											<Button
												type="button"
												className="timecode-instance-grab"
												onPointerDown={(event) =>
													begin(event, {
														kind: "instance",
														startValue: instance.start,
														laneId: lane.id,
														instanceId: instance.id,
													})
												}
											>
												<strong>
													{lane.label.replace(/^Cuelist \d+ · /, "")} · Take{" "}
													{instanceIndex + 1}
												</strong>
												<small>
													{formatTimecode(instance.start)} →{" "}
													{formatTimecode(instance.end)}
												</small>
											</Button>
											{instance.cues.map((cue) => {
												const selected =
													selection?.laneId === lane.id &&
													selection.instanceId === instance.id &&
													selection.pointId === cue.id;
												return (
													<div
														className={`timecode-cue ${selected ? "selected" : ""}`}
														key={cue.id}
														style={{ left: framePx(cue.go) }}
													>
														<div
															className="timecode-cue-fade"
															style={{
																width: Math.max(12, framePx(cue.fade)),
															}}
														/>
														<Button
															type="button"
															className="timecode-go-handle"
															aria-label={`Cue ${cue.number} ${cue.name} Go at ${formatTimecode(instance.start + cue.go)}`}
															title={`Cue ${cue.number} · ${cue.name}`}
															onClick={() => {
																onSelection(cueSelection(lane, instance, cue));
																onFrame(instance.start + cue.go);
															}}
															onPointerDown={(event) => {
																onSelection(cueSelection(lane, instance, cue));
																begin(event, {
																	kind: "cue-go",
																	startValue: cue.go,
																	laneId: lane.id,
																	instanceId: instance.id,
																	pointId: cue.id,
																});
															}}
														>
															<span>{cue.number}</span>
														</Button>
														<Button
															type="button"
															className="timecode-fade-handle"
															style={{
																left: Math.max(12, framePx(cue.fade)),
															}}
															aria-label={`Cue ${cue.number} fade ${formatTimecode(cue.fade)}`}
															title={`Fade ${formatTimecode(cue.fade)}`}
															onPointerDown={(event) => {
																onSelection(cueSelection(lane, instance, cue));
																begin(event, {
																	kind: "cue-fade",
																	startValue: cue.fade,
																	laneId: lane.id,
																	instanceId: instance.id,
																	pointId: cue.id,
																});
															}}
														/>
													</div>
												);
											})}
											<Button
												type="button"
												className="timecode-instance-end"
												aria-label={`Resize end of ${lane.label}`}
												title="Drag Cuelist end"
												onPointerDown={(event) =>
													begin(event, {
														kind: "instance-end",
														startValue: instance.end,
														laneId: lane.id,
														instanceId: instance.id,
													})
												}
											/>
										</div>
									))}
									{lane.points?.map((point) => {
										const selected =
											selection?.laneId === lane.id &&
											selection.pointId === point.id;
										return (
											<div
												className={`timecode-automation ${selected ? "selected" : ""}`}
												key={point.id}
												style={
													{
														left: framePx(point.frame),
														"--lane-color": lane.color,
													} as CSSProperties
												}
											>
												{point.fade > 0 && (
													<div
														className="timecode-automation-fade"
														style={{ width: framePx(point.fade) }}
													/>
												)}
												<Button
													type="button"
													className="timecode-automation-handle"
													aria-label={`${lane.label} ${point.value}${lane.kind === "speed-group" ? " BPM" : "%"}`}
													onClick={() => {
														onSelection(automationSelection(lane, point));
														onFrame(point.frame);
													}}
													onPointerDown={(event) => {
														onSelection(automationSelection(lane, point));
														begin(event, {
															kind: "automation",
															startValue: point.frame,
															laneId: lane.id,
															pointId: point.id,
														});
													}}
												>
													<strong>
														{point.value}
														{lane.kind === "speed-group" ? " BPM" : "%"}
													</strong>
													<small>
														{lane.kind === "speed-group"
															? point.restart
																? "Restart · Beat 1"
																: "Keep running"
															: `${(point.fade / TIMECODE_HZ).toFixed(1)}s fade`}
													</small>
												</Button>
												{point.fade > 0 && (
													<Button
														type="button"
														className="timecode-automation-fade-handle"
														style={{ left: framePx(point.fade) }}
														aria-label={`${lane.label} fade end`}
														onPointerDown={(event) => {
															onSelection(automationSelection(lane, point));
															begin(event, {
																kind: "automation-fade",
																startValue: point.fade,
																laneId: lane.id,
																pointId: point.id,
															});
														}}
													/>
												)}
											</div>
										);
									})}
								</section>
							</div>
						))}
						<div
							className="timecode-loop-range"
							role="img"
							style={{
								left: 196 + framePx(loopStart),
								width: Math.max(1, framePx(loopEnd - loopStart)),
							}}
							aria-label={`Loop ${formatTimecode(loopStart)} to ${formatTimecode(loopEnd)}`}
						>
							<span>START</span>
							<span>END</span>
						</div>
						<div
							className="timecode-playhead"
							style={{ left: 196 + framePx(frame) }}
						>
							<Button
								type="button"
								aria-label={`Playhead ${formatTimecode(frame)}`}
								onPointerDown={(event) =>
									begin(event, {
										kind: "playhead",
										startValue: frame,
									})
								}
							/>
						</div>
					</div>
				</section>
				<footer className="timecode-control-bar">
					<nav className="timecode-transport" aria-label="Timecode transport">
						<TransportButton
							label="Jump to beginning"
							onClick={() => onFrame(loopStart)}
						>
							|◀
						</TransportButton>
						<TransportButton
							label="Rewind"
							active={playing && rate < 0}
							onClick={() => {
								onRate(-Math.max(1, Math.abs(rate)));
								onPlaying(true);
							}}
						>
							◀◀
						</TransportButton>
						<TransportButton
							label="Stop"
							active={!playing}
							onClick={() => onPlaying(false)}
						>
							■
						</TransportButton>
						<TransportButton
							label="Play"
							active={playing && rate > 0}
							onClick={() => {
								onRate(Math.max(1, rate));
								onPlaying(true);
							}}
						>
							▶
						</TransportButton>
						<TransportButton
							label="Play slower"
							onClick={() => {
								onRate(Math.max(0.25, Math.abs(rate) / 2));
								onPlaying(true);
							}}
						>
							½▶
						</TransportButton>
						<TransportButton
							label="Play faster"
							onClick={() => {
								onRate(Math.min(4, Math.max(1, Math.abs(rate) * 2)));
								onPlaying(true);
							}}
						>
							▶▶
						</TransportButton>
						<Button size="compact" onClick={() => onRate(1)}>
							{rate > 0 ? "+" : "−"}
							{Math.abs(rate)}×
						</Button>
					</nav>
					<Button
						type="button"
						className="timecode-position"
						onClick={() => {
							const value = window.prompt(
								`Jump to frame (0–${TIMECODE_TOTAL_FRAMES})`,
								String(Math.round(frame)),
							);
							if (value !== null) onFrame(clampFrame(Number(value)));
						}}
					>
						<strong>{formatTimecode(frame)}</strong>
						<small>
							Playhead · Frame {Math.round(frame)} · {TIMECODE_HZ} Hz
						</small>
					</Button>
					<div className="timecode-loop-readout">
						<strong>
							{formatTimecode(loopStart)} → {formatTimecode(loopEnd)}
						</strong>
						<small>Loop range · controlled by Timeline encoders</small>
					</div>
					<div className="timecode-audio-readout">
						<span>♫</span>
						<div>
							<strong>{audioFile ?? "No song assigned"}</strong>
							<small>Follows playhead and loop</small>
						</div>
					</div>
				</footer>
			</div>
		</section>
	);
}
