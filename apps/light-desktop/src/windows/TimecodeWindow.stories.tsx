/**
 * FUTURE FEATURE — STORYBOOK PRODUCT-DESIGN PROTOTYPE.
 *
 * This story intentionally uses deterministic local state. It demonstrates the
 * proposed Timecode interaction without implementing runtime or persistence.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import {
	Button,
	EncoderSection,
	type EncoderSectionModel,
} from "@tosklight/ui";
import { GridDesktop, PaneView } from "@tosklight/ui/desktop";
import { useMemo, useState } from "react";
import { CommandSectionFixture } from "../../../ui-library/storybook/fixtures/controlSection";
import { ApplicationStateHarness } from "../../../ui-library/storybook/providers/ApplicationStateHarness";
import { AppShellView } from "../components/shell/AppShell";
import { Clock } from "../components/shell/Clock";
import { LeftDock } from "../components/shell/LeftDock";
import {
	formatTimecode,
	secondsToFrames,
	TIMECODE_HZ,
	TIMECODE_TOTAL_FRAMES,
	type TimecodeLane,
	type TimecodeSelection,
	TimecodeWindow,
} from "./TimecodeWindow";

const meta = {
	title: "ToskLight/Windows/Timecode",
	tags: ["autodocs"],
	parameters: {
		layout: "fullscreen",
		docs: {
			description: {
				component:
					"Storybook-only product-design prototype for a frame-addressed Timecode editor. Audio, transport, lanes, and control points are deterministic local demo state.",
			},
		},
	},
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;

const initialLanes: TimecodeLane[] = [
	{
		id: "opening",
		kind: "cuelist",
		objectId: "cuelist-1",
		label: "Cuelist 1 · Opening",
		color: "#a9e34b",
		instances: [
			{
				id: "opening-1",
				start: secondsToFrames(10),
				end: secondsToFrames(28),
				cues: [
					{
						id: "opening-1-1",
						number: "1",
						name: "Preset",
						go: 0,
						fade: secondsToFrames(2),
					},
					{
						id: "opening-1-2",
						number: "2",
						name: "Rise",
						go: secondsToFrames(4),
						fade: secondsToFrames(3.5),
					},
					{
						id: "opening-1-3",
						number: "3",
						name: "Hit",
						go: secondsToFrames(10),
						fade: secondsToFrames(0.25),
					},
					{
						id: "opening-1-4",
						number: "4",
						name: "Resolve",
						go: secondsToFrames(14),
						fade: secondsToFrames(4),
					},
				],
			},
			{
				id: "opening-2",
				start: secondsToFrames(33),
				end: secondsToFrames(51),
				cues: [
					{
						id: "opening-2-1",
						number: "1",
						name: "Preset",
						go: 0,
						fade: secondsToFrames(1),
					},
					{
						id: "opening-2-2",
						number: "2",
						name: "Rise",
						go: secondsToFrames(5),
						fade: secondsToFrames(2.25),
					},
					{
						id: "opening-2-3",
						number: "3",
						name: "Hit",
						go: secondsToFrames(9),
						fade: secondsToFrames(0.5),
					},
					{
						id: "opening-2-4",
						number: "4",
						name: "Resolve",
						go: secondsToFrames(13),
						fade: secondsToFrames(5),
					},
				],
			},
		],
	},
	{
		id: "accents",
		kind: "cuelist",
		objectId: "cuelist-2",
		label: "Cuelist 2 · Accents",
		color: "#63d986",
		instances: [
			{
				id: "accents-1",
				start: secondsToFrames(15),
				end: secondsToFrames(43),
				cues: [
					{
						id: "accents-1-1",
						number: "1",
						name: "Ready",
						go: 0,
						fade: secondsToFrames(1.5),
					},
					{
						id: "accents-1-2",
						number: "2",
						name: "Beat accent",
						go: secondsToFrames(7.5),
						fade: secondsToFrames(0.2),
					},
					{
						id: "accents-1-3",
						number: "3",
						name: "Sweep",
						go: secondsToFrames(16),
						fade: secondsToFrames(3),
					},
				],
			},
		],
	},
	{
		id: "front-master",
		kind: "group-master",
		objectId: "group-1",
		label: "Group 1 · Front Wash",
		color: "#f5bd4f",
		points: [
			{
				id: "front-1",
				frame: secondsToFrames(6),
				value: 35,
				fade: secondsToFrames(3),
			},
			{
				id: "front-2",
				frame: secondsToFrames(19),
				value: 100,
				fade: secondsToFrames(5),
			},
			{
				id: "front-3",
				frame: secondsToFrames(46),
				value: 20,
				fade: secondsToFrames(7),
			},
		],
	},
	{
		id: "speed-a",
		kind: "speed-group",
		objectId: "speed-group-a",
		label: "Speed Group A",
		color: "#ad8bff",
		points: [
			{
				id: "speed-1",
				frame: secondsToFrames(8),
				value: 120,
				fade: 0,
				restart: true,
			},
			{
				id: "speed-2",
				frame: secondsToFrames(31),
				value: 96,
				fade: 0,
				restart: false,
			},
			{
				id: "speed-3",
				frame: secondsToFrames(49),
				value: 132,
				fade: 0,
				restart: true,
			},
		],
	},
];

interface ResolvedSelection {
	selection: TimecodeSelection;
	frame: number;
	fade: number;
	label: string;
	color: string;
	kind: TimecodeLane["kind"];
	value?: number;
	restart?: boolean;
}

function orderedPoints(lanes: TimecodeLane[]) {
	const points: ResolvedSelection[] = [];
	for (const lane of lanes) {
		for (const instance of lane.instances ?? []) {
			for (const cue of instance.cues) {
				points.push({
					selection: {
						laneId: lane.id,
						instanceId: instance.id,
						pointId: cue.id,
					},
					frame: instance.start + cue.go,
					fade: cue.fade,
					label: `${lane.label} · Cue ${cue.number} ${cue.name}`,
					color: lane.color,
					kind: lane.kind,
				});
			}
		}
		for (const point of lane.points ?? []) {
			points.push({
				selection: { laneId: lane.id, pointId: point.id },
				frame: point.frame,
				fade: point.fade,
				label: `${lane.label} · ${point.value}${lane.kind === "speed-group" ? " BPM" : "%"}`,
				color: lane.color,
				kind: lane.kind,
				value: point.value,
				restart: point.restart,
			});
		}
	}
	return points.sort((left, right) => left.frame - right.frame);
}

function sameSelection(
	left: TimecodeSelection,
	right: TimecodeSelection | null,
) {
	return (
		left.laneId === right?.laneId &&
		left.instanceId === right.instanceId &&
		left.pointId === right.pointId
	);
}

function updateSelected(
	lanes: TimecodeLane[],
	selection: TimecodeSelection,
	update: {
		frame?: number;
		fade?: number;
		value?: number;
		restart?: boolean;
	},
) {
	return lanes.map((lane) => {
		if (lane.id !== selection.laneId) return lane;
		if (selection.instanceId) {
			return {
				...lane,
				instances: lane.instances?.map((instance) => {
					if (instance.id !== selection.instanceId) return instance;
					return {
						...instance,
						cues: instance.cues.map((cue) =>
							cue.id !== selection.pointId
								? cue
								: {
										...cue,
										...(update.frame === undefined
											? {}
											: {
													go: Math.max(
														0,
														Math.min(
															instance.end - instance.start,
															update.frame - instance.start,
														),
													),
												}),
										...(update.fade === undefined
											? {}
											: { fade: Math.max(0, update.fade) }),
									},
						),
					};
				}),
			};
		}
		return {
			...lane,
			points: lane.points?.map((point) =>
				point.id !== selection.pointId
					? point
					: {
							...point,
							...(update.frame === undefined
								? {}
								: { frame: Math.max(0, update.frame) }),
							...(update.fade === undefined
								? {}
								: { fade: Math.max(0, update.fade) }),
							...(update.value === undefined ? {} : { value: update.value }),
							...(update.restart === undefined
								? {}
								: { restart: update.restart }),
						},
			),
		};
	});
}

function TimecodeEncoderDeck({
	hardware,
	lanes,
	selection,
	frame,
	zoom,
	verticalZoom,
	loopStart,
	loopEnd,
	onLanes,
	onSelection,
	onFrame,
	onZoom,
	onVerticalZoom,
	onLoopStart,
	onLoopEnd,
}: {
	hardware: boolean;
	lanes: TimecodeLane[];
	selection: TimecodeSelection;
	frame: number;
	zoom: number;
	verticalZoom: number;
	loopStart: number;
	loopEnd: number;
	onLanes(lanes: TimecodeLane[]): void;
	onSelection(selection: TimecodeSelection): void;
	onFrame(frame: number): void;
	onZoom(zoom: number): void;
	onVerticalZoom(zoom: number): void;
	onLoopStart(frame: number): void;
	onLoopEnd(frame: number): void;
}) {
	const [encoderGroup, setEncoderGroup] = useState<"timeline" | "point">(
		"timeline",
	);
	const points = useMemo(() => orderedPoints(lanes), [lanes]);
	const selectedIndex = Math.max(
		0,
		points.findIndex((point) => sameSelection(point.selection, selection)),
	);
	const selected = points[selectedIndex];
	const pointModel: EncoderSectionModel = {
		id: "timecode-point",
		label: "Control Point",
		description: selected?.label ?? "Select a control point",
		encoders: [
			{
				id: "control-point",
				slot: 1,
				target: {
					label: "Control point",
					display: `${selectedIndex + 1} / ${points.length}`,
					role: "Previous / next",
				},
				value: selectedIndex,
				minimum: 0,
				maximum: Math.max(0, points.length - 1),
				inputScale: 1,
				slowStep: 1,
				fastStep: 1,
				touchInteraction: "choices",
				presets: {
					selectedValue: String(selectedIndex),
					groups: [
						{
							label: "Timeline control points",
							options: points.map((point, index) => ({
								value: String(index),
								label: `${index + 1} · ${point.label}`,
							})),
						},
					],
				},
				accentColor: selected?.color ?? "#58d4ef",
			},
			{
				id: "go",
				slot: 2,
				target: {
					label: "Go / change point",
					display: selected ? formatTimecode(selected.frame) : "—",
					role: "Move point",
				},
				value: selected?.frame ?? 0,
				minimum: 0,
				maximum: TIMECODE_TOTAL_FRAMES,
				inputScale: 1,
				slowStep: 1,
				fastStep: TIMECODE_HZ,
				disabled: !selected,
				mode: "1 frame",
				accentColor: selected?.color ?? "#58d4ef",
			},
			{
				id: "fade",
				slot: 3,
				target: {
					label: "Fade time",
					display: selected ? formatTimecode(selected.fade) : "—",
					role: "Set duration",
				},
				value: selected?.fade ?? 0,
				minimum: 0,
				maximum: secondsToFrames(30),
				inputScale: 1,
				slowStep: 1,
				fastStep: TIMECODE_HZ,
				disabled: !selected,
				mode: "1 frame",
				accentColor: selected?.color ?? "#58d4ef",
			},
			{
				id: "value",
				slot: 4,
				target:
					selected && selected.kind !== "cuelist"
						? {
								label:
									selected.kind === "speed-group"
										? "Speed"
										: "Group Master value",
								display: `${selected.value}${selected.kind === "speed-group" ? " BPM" : "%"}`,
								role: "Set value",
							}
						: undefined,
				value: selected?.value ?? 0,
				minimum: selected?.kind === "speed-group" ? 1 : 0,
				maximum: selected?.kind === "speed-group" ? 999 : 100,
				inputScale: 1,
				slowStep: 1,
				fastStep: selected?.kind === "speed-group" ? 5 : 10,
				disabled: !selected || selected.kind === "cuelist",
				accentColor: selected?.color ?? "#58d4ef",
			},
			{
				id: "run-behavior",
				slot: 5,
				target:
					selected?.kind === "speed-group"
						? {
								label: "Beat behavior",
								display: selected.restart ? "Restart · Beat 1" : "Keep running",
								role: "Continue / restart",
							}
						: undefined,
				value: selected?.restart ? 1 : 0,
				minimum: 0,
				maximum: 1,
				inputScale: 1,
				slowStep: 1,
				fastStep: 1,
				disabled: selected?.kind !== "speed-group",
				touchInteraction: "choices",
				presets: {
					selectedValue: selected?.restart ? "1" : "0",
					groups: [
						{
							label: "Beat behavior",
							options: [
								{ value: "0", label: "Keep running" },
								{ value: "1", label: "Restart · Beat 1" },
							],
						},
					],
				},
				accentColor: selected?.color ?? "#58d4ef",
			},
			{
				id: "point-unused",
				slot: 6,
				value: 0,
				disabled: true,
			},
		],
	};
	const timelineModel: EncoderSectionModel = {
		id: "timecode-timeline",
		label: "Timeline",
		description: `${formatTimecode(loopStart)} → ${formatTimecode(loopEnd)}`,
		encoders: [
			{
				id: "timeline-zoom",
				slot: 1,
				target: {
					label: "Horizontal zoom",
					display: `${Math.round(zoom * 100)}%`,
					role: "Scale time",
				},
				value: zoom,
				minimum: 0.55,
				maximum: 4.5,
				inputScale: 100,
				slowStep: 0.05,
				fastStep: 0.25,
				accentColor: "#58d4ef",
			},
			{
				id: "playhead",
				slot: 2,
				target: {
					label: "Playhead",
					display: formatTimecode(frame),
					role: "Navigate",
				},
				value: frame,
				minimum: 0,
				maximum: TIMECODE_TOTAL_FRAMES,
				inputScale: 1,
				slowStep: 1,
				fastStep: TIMECODE_HZ,
				mode: "1 frame · 44 Hz",
				accentColor: "#ff5a5f",
			},
			{
				id: "loop-start",
				slot: 3,
				target: {
					label: "Loop start",
					display: formatTimecode(loopStart),
					role: "Set range start",
				},
				value: loopStart,
				minimum: 0,
				maximum: loopEnd - 1,
				inputScale: 1,
				slowStep: 1,
				fastStep: TIMECODE_HZ,
				mode: "1 frame",
				accentColor: "#4edcff",
			},
			{
				id: "loop-end",
				slot: 4,
				target: {
					label: "Loop end",
					display: formatTimecode(loopEnd),
					role: "Set range end",
				},
				value: loopEnd,
				minimum: loopStart + 1,
				maximum: TIMECODE_TOTAL_FRAMES,
				inputScale: 1,
				slowStep: 1,
				fastStep: TIMECODE_HZ,
				mode: "1 frame",
				accentColor: "#4edcff",
			},
			{
				id: "vertical-zoom",
				slot: 5,
				target: {
					label: "Vertical zoom",
					display: `${Math.round(verticalZoom * 100)}%`,
					role: "Scale lanes",
				},
				value: verticalZoom,
				minimum: 0.7,
				maximum: 1.75,
				inputScale: 100,
				slowStep: 0.05,
				fastStep: 0.2,
				accentColor: "#58d4ef",
			},
			{
				id: "timeline-unused",
				slot: 6,
				value: 0,
				disabled: true,
			},
		],
	};
	const model = encoderGroup === "timeline" ? timelineModel : pointModel;

	const choosePoint = (index: number) => {
		const next =
			points[Math.max(0, Math.min(points.length - 1, Math.round(index)))];
		if (!next) return;
		onSelection(next.selection);
		onFrame(next.frame);
	};

	const updateFrame = (value: number) => {
		if (!selected) return;
		const requested = Math.max(
			0,
			Math.min(TIMECODE_TOTAL_FRAMES, Math.round(value)),
		);
		const lane = lanes.find(
			(candidate) => candidate.id === selected.selection.laneId,
		);
		const instance = lane?.instances?.find(
			(candidate) => candidate.id === selected.selection.instanceId,
		);
		const next = instance
			? Math.max(instance.start, Math.min(instance.end, requested))
			: requested;
		onLanes(updateSelected(lanes, selected.selection, { frame: next }));
		onFrame(next);
	};

	const updateFade = (value: number) => {
		if (!selected) return;
		onLanes(
			updateSelected(lanes, selected.selection, {
				fade: Math.max(0, Math.min(secondsToFrames(30), Math.round(value))),
			}),
		);
	};

	const updateValue = (value: number) => {
		if (!selected || selected.kind === "cuelist") return;
		const maximum = selected.kind === "speed-group" ? 999 : 100;
		onLanes(
			updateSelected(lanes, selected.selection, {
				value: Math.max(
					selected.kind === "speed-group" ? 1 : 0,
					Math.min(maximum, Math.round(value)),
				),
			}),
		);
	};

	const updateRestart = (restart: boolean) => {
		if (!selected || selected.kind !== "speed-group") return;
		onLanes(updateSelected(lanes, selected.selection, { restart }));
	};

	return (
		<div className="parameter-controls">
			<div className="family-tabs timecode-programmer-tabs">
				<Button
					active={encoderGroup === "timeline"}
					onClick={() => setEncoderGroup("timeline")}
				>
					Timeline
				</Button>
				<Button
					active={encoderGroup === "point"}
					onClick={() => setEncoderGroup("point")}
				>
					Control Point
				</Button>
				<span>
					{encoderGroup === "timeline"
						? `Loop ${formatTimecode(loopStart)} → ${formatTimecode(loopEnd)}`
						: (selected?.label ?? "Select a control point")}
				</span>
			</div>
			<div className="parameter-surfaces">
				<EncoderSection
					className="timecode-encoder-section"
					model={model}
					surface={hardware ? "hardware" : "touch"}
					showHeader={false}
					callbacks={{
						onRelativeChange: (id, delta) => {
							if (id === "timeline-zoom")
								onZoom(Math.max(0.55, Math.min(4.5, zoom + delta)));
							if (id === "playhead")
								onFrame(
									Math.max(0, Math.min(TIMECODE_TOTAL_FRAMES, frame + delta)),
								);
							if (id === "loop-start")
								onLoopStart(
									Math.max(0, Math.min(loopEnd - 1, loopStart + delta)),
								);
							if (id === "loop-end")
								onLoopEnd(
									Math.max(
										loopStart + 1,
										Math.min(TIMECODE_TOTAL_FRAMES, loopEnd + delta),
									),
								);
							if (id === "vertical-zoom")
								onVerticalZoom(
									Math.max(0.7, Math.min(1.75, verticalZoom + delta)),
								);
							if (id === "control-point") choosePoint(selectedIndex + delta);
							if (id === "go") updateFrame((selected?.frame ?? 0) + delta);
							if (id === "fade") updateFade((selected?.fade ?? 0) + delta);
							if (id === "value") updateValue((selected?.value ?? 0) + delta);
							if (id === "run-behavior")
								updateRestart(
									Math.round((selected?.restart ? 1 : 0) + delta) > 0,
								);
						},
						onAbsoluteChange: (id, value) => {
							if (id === "timeline-zoom")
								onZoom(Math.max(0.55, Math.min(4.5, value)));
							if (id === "playhead") onFrame(value);
							if (id === "loop-start")
								onLoopStart(Math.max(0, Math.min(loopEnd - 1, value)));
							if (id === "loop-end")
								onLoopEnd(
									Math.max(
										loopStart + 1,
										Math.min(TIMECODE_TOTAL_FRAMES, value),
									),
								);
							if (id === "vertical-zoom")
								onVerticalZoom(Math.max(0.7, Math.min(1.75, value)));
							if (id === "go") updateFrame(value);
							if (id === "fade") updateFade(value);
							if (id === "value") updateValue(value);
						},
						onPresetSelect: (id, value) => {
							if (id === "control-point") choosePoint(Number(value));
							if (id === "run-behavior") updateRestart(value === "1");
						},
					}}
				/>
			</div>
		</div>
	);
}

function FullApplicationTimecode({
	hardware,
	marketing = false,
}: {
	hardware: boolean;
	marketing?: boolean;
}) {
	const [lanes, setLanes] = useState(initialLanes);
	const [selection, setSelection] = useState<TimecodeSelection>({
		laneId: "opening",
		instanceId: "opening-1",
		pointId: "opening-1-3",
	});
	const [frame, setFrame] = useState(secondsToFrames(20));
	const [playing, setPlaying] = useState(false);
	const [rate, setRate] = useState(1);
	const [zoom, setZoom] = useState(1.55);
	const [verticalZoom, setVerticalZoom] = useState(1);
	const [loopStart, setLoopStart] = useState(secondsToFrames(10));
	const [loopEnd, setLoopEnd] = useState(secondsToFrames(51));
	const [audioFile, setAudioFile] = useState<string | null>(
		"Midnight Drive.wav",
	);

	return (
		<ApplicationStateHarness>
			<AppShellView
				dock={
					<LeftDock
						presentation={{
							showIdentity: marketing ? "Demo Show" : "Timecode UI Review",
							showIndicator: {
								label: marketing ? "Demo show" : "Offline mock",
								detail: marketing
									? "Deterministic marketing presentation."
									: "Timeline changes stay in Storybook memory",
								className: marketing
									? "show-status-connected"
									: "show-status-warning",
								connected: marketing,
							},
							clock: <Clock now={new Date(2026, 6, 29, 20, 14, 22)} />,
						}}
					/>
				}
				workspace={
					<GridDesktop id="timecode-review" name="Timecode Review">
						<PaneView
							maximized
							showHeader={false}
							pane={{
								id: "timecode",
								title: "Timecode",
								type: "timecode",
								x: 1,
								y: 1,
								width: 24,
								height: 18,
							}}
						>
							<TimecodeWindow
								lanes={lanes}
								selection={selection}
								frame={frame}
								playing={playing}
								rate={rate}
								zoom={zoom}
								verticalZoom={verticalZoom}
								loopStart={loopStart}
								loopEnd={loopEnd}
								audioFile={audioFile}
								onLanes={setLanes}
								onSelection={setSelection}
								onFrame={setFrame}
								onPlaying={setPlaying}
								onRate={setRate}
								onZoom={setZoom}
								onVerticalZoom={setVerticalZoom}
								onAudioFile={setAudioFile}
							/>
						</PaneView>
					</GridDesktop>
				}
				control={
					<CommandSectionFixture
						inheritAppState
						initialMode="programmer"
						hardware={hardware}
						programmer={
							<TimecodeEncoderDeck
								hardware={hardware}
								lanes={lanes}
								selection={selection}
								frame={frame}
								zoom={zoom}
								verticalZoom={verticalZoom}
								loopStart={loopStart}
								loopEnd={loopEnd}
								onLanes={setLanes}
								onSelection={setSelection}
								onFrame={setFrame}
								onZoom={setZoom}
								onVerticalZoom={setVerticalZoom}
								onLoopStart={setLoopStart}
								onLoopEnd={setLoopEnd}
							/>
						}
					/>
				}
			/>
		</ApplicationStateHarness>
	);
}

export function MarketingTimecodeApplication() {
	return <FullApplicationTimecode hardware={false} marketing />;
}

export const FullApplicationDiscussion: Story = {
	render: (_args, context) => (
		<FullApplicationTimecode hardware={context.globals.mode === "hardware"} />
	),
};
