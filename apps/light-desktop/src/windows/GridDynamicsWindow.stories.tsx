/**
 * Intentional Storybook product-design surface for future Grid Dynamics.
 * The complete editor is deterministic local state and never contacts a desk.
 */
import type { Meta, StoryObj } from "@storybook/react-vite";
import { Button } from "@tosklight/ui";
import { GridDesktop, PaneView } from "@tosklight/ui/desktop";
import {
	EncoderSection,
	type EncoderSectionModel,
} from "@tosklight/ui/encoders";
import { useMemo, useState } from "react";
import { CommandSectionFixture } from "../../../ui-library/storybook/fixtures/controlSection";
import { ApplicationStateHarness } from "../../../ui-library/storybook/providers/ApplicationStateHarness";
import { AppShellView } from "../components/shell/AppShell";
import { Clock } from "../components/shell/Clock";
import { LeftDock } from "../components/shell/LeftDock";
import {
	createGridGroup,
	type GridCellSelection,
	type GridDynamicConfig,
	type GridDynamicGroup,
	GridDynamicsWindow,
	gridPresets,
	initialGroupChoices,
} from "./GridDynamicsWindow";

const meta = {
	title: "ToskLight/Windows/Grid Dynamics",
	tags: ["autodocs"],
	parameters: {
		layout: "fullscreen",
		docs: {
			description: {
				component:
					"Storybook-only product-design mock for a group-based, drum-machine-style Dynamic editor. Preset painting, tile timing, grid and speed configuration stay in local state.",
			},
		},
	},
} satisfies Meta;

export default meta;
type Story = StoryObj<typeof meta>;
type EncoderPage = "steps" | "grid" | "speed";

function seededGroups(columns: number): GridDynamicGroup[] {
	const groups = initialGroupChoices
		.slice(0, 3)
		.map((choice) => createGridGroup(choice, columns));
	const movers = groups[1];
	if (movers) {
		movers.lanes.splice(1, 0, {
			id: "moving-heads-position",
			kind: "position",
			label: "Position",
			cells: Array.from({ length: columns }, () => ({
				preset: null,
				alternate: null,
				attack: 0,
				decay: 0,
			})),
		});
	}
	const patterns: Array<{
		group: number;
		lane: number;
		preset: string;
		steps: number[];
		attack?: number;
		decay?: number;
	}> = [
		{
			group: 0,
			lane: 0,
			preset: "intensity-full",
			steps: [0, 4, 8, 12],
			attack: 12,
			decay: 35,
		},
		{
			group: 0,
			lane: 1,
			preset: "color-cyan",
			steps: [0, 1, 4, 5, 8, 9, 12, 13],
		},
		{
			group: 0,
			lane: 1,
			preset: "color-magenta",
			steps: [2, 3, 6, 7, 10, 11, 14, 15],
		},
		{
			group: 1,
			lane: 0,
			preset: "intensity-full",
			steps: [0, 2, 6, 8, 10, 14],
			attack: 8,
			decay: 20,
		},
		{
			group: 1,
			lane: 1,
			preset: "position-fan",
			steps: [0, 4, 8, 12],
			attack: 30,
			decay: 30,
		},
		{ group: 1, lane: 1, preset: "position-cross", steps: [2, 6, 10, 14] },
		{ group: 1, lane: 2, preset: "color-magenta", steps: [0, 4, 8, 12] },
		{ group: 1, lane: 2, preset: "color-blue", steps: [2, 6, 10, 14] },
		{
			group: 2,
			lane: 0,
			preset: "intensity-50",
			steps: [1, 3, 5, 7, 9, 11, 13, 15],
			decay: 18,
		},
		{ group: 2, lane: 1, preset: "color-amber", steps: [0, 4, 8, 12] },
		{ group: 2, lane: 1, preset: "color-cyan", steps: [2, 6, 10, 14] },
	];
	for (const pattern of patterns) {
		const lane = groups[pattern.group]?.lanes[pattern.lane];
		if (!lane) continue;
		for (const step of pattern.steps) {
			lane.cells[step] = {
				preset: pattern.preset,
				alternate: null,
				attack: pattern.attack ?? 0,
				decay: pattern.decay ?? 0,
			};
		}
	}
	return groups;
}

function clamp(value: number, minimum: number, maximum: number) {
	return Math.max(minimum, Math.min(maximum, value));
}

function GridDynamicsEncoderDeck({
	hardware,
	groups,
	config,
	activePreset,
	selection,
	playing,
	pendingPlaying,
	playhead,
	onGroups,
	onConfig,
	onActivePreset,
	onSelection,
	onPlaying,
	onPlayhead,
}: {
	hardware: boolean;
	groups: GridDynamicGroup[];
	config: GridDynamicConfig;
	activePreset: string;
	selection: GridCellSelection;
	playing: boolean;
	pendingPlaying: boolean;
	playhead: number;
	onGroups(groups: GridDynamicGroup[]): void;
	onConfig(config: GridDynamicConfig): void;
	onActivePreset(id: string): void;
	onSelection(selection: GridCellSelection): void;
	onPlaying(playing: boolean): void;
	onPlayhead(step: number): void;
}) {
	const [page, setPage] = useState<EncoderPage>("steps");
	const [firstVisible, setFirstVisible] = useState(1);
	const [zoom, setZoom] = useState(1);
	const [rangeStart, setRangeStart] = useState(1);
	const [rangeEnd, setRangeEnd] = useState(config.columns);
	const groupIndex = Math.max(
		0,
		groups.findIndex((group) => group.id === selection.groupId),
	);
	const group = groups[groupIndex] ?? groups[0];
	const laneIndex = Math.max(
		0,
		group?.lanes.findIndex((lane) => lane.id === selection.laneId) ?? 0,
	);
	const lane = group?.lanes[laneIndex];
	const cell = lane?.cells[selection.column];
	const compatiblePresets = gridPresets.filter(
		(preset) => preset.kind === lane?.kind,
	);
	const presetIndex = Math.max(
		0,
		compatiblePresets.findIndex((preset) => preset.id === activePreset),
	);
	const groupChoices = groups.map((candidate, index) => ({
		value: String(index),
		label: `${candidate.number} · ${candidate.name}`,
	}));
	const laneChoices = (group?.lanes ?? []).map((candidate, index) => ({
		value: String(index),
		label: candidate.label,
	}));
	const presetChoices = compatiblePresets.map((preset, index) => ({
		value: String(index),
		label: preset.label,
	}));
	const choicePreset = (
		selectedValue: number,
		label: string,
		options: Array<{ value: string; label: string }>,
	) => ({
		selectedValue: String(selectedValue),
		groups: [{ label, options }],
	});
	const stepsModel: EncoderSectionModel = {
		id: "grid-dynamics-steps",
		label: "Steps",
		description: `${group?.name ?? "—"} · ${lane?.label ?? "—"} · Step ${selection.column + 1}`,
		encoders: [
			{
				id: "group",
				slot: 1,
				target: {
					label: "Group",
					display: group ? `${group.number} · ${group.name}` : "—",
					role: "Select group",
				},
				value: groupIndex,
				minimum: 0,
				maximum: Math.max(0, groups.length - 1),
				inputScale: 1,
				slowStep: 1,
				fastStep: 1,
				touchInteraction: "choices",
				presets: choicePreset(groupIndex, "Groups", groupChoices),
				accentColor: group?.color,
			},
			{
				id: "lane",
				slot: 2,
				target: {
					label: "Lane",
					display: lane?.label ?? "—",
					role: "Select lane",
				},
				value: laneIndex,
				minimum: 0,
				maximum: Math.max(0, (group?.lanes.length ?? 1) - 1),
				inputScale: 1,
				slowStep: 1,
				fastStep: 1,
				touchInteraction: "choices",
				presets: choicePreset(laneIndex, "Lanes", laneChoices),
				accentColor:
					lane?.kind === "intensity"
						? "#f6c453"
						: lane?.kind === "position"
							? "#55c7ef"
							: "#ef5ac8",
			},
			{
				id: "step",
				slot: 3,
				target: {
					label: "Step",
					display: `${selection.column + 1} / ${config.columns}`,
					role: "Select tile",
				},
				value: selection.column,
				minimum: 0,
				maximum: config.columns - 1,
				inputScale: 1,
				slowStep: 1,
				fastStep: 4,
				mode: "1 step",
				accentColor: "#ff5d6d",
			},
			{
				id: "preset",
				slot: 4,
				target: {
					label: "Preset / Brush",
					display: compatiblePresets[presetIndex]?.label ?? "Current",
					role: "Paint preset",
				},
				value: presetIndex,
				minimum: 0,
				maximum: Math.max(0, compatiblePresets.length - 1),
				inputScale: 1,
				slowStep: 1,
				fastStep: 1,
				touchInteraction: "choices",
				presets: choicePreset(
					presetIndex,
					`${lane?.label ?? "Lane"} presets`,
					presetChoices,
				),
				accentColor: compatiblePresets[presetIndex]?.color,
			},
			{
				id: "attack",
				slot: 5,
				target: {
					label: "Attack",
					display: `${cell?.attack ?? 0}%`,
					role: "Fade into preset",
				},
				value: (cell?.attack ?? 0) / 100,
				minimum: 0,
				maximum: 1,
				inputScale: 100,
				slowStep: 0.01,
				fastStep: 0.1,
				accentColor: "#64d5a3",
			},
			{
				id: "decay",
				slot: 6,
				target: {
					label: "Decay",
					display: `${cell?.decay ?? 0}%`,
					role: "Fade to next preset",
				},
				value: (cell?.decay ?? 0) / 100,
				minimum: 0,
				maximum: 1,
				inputScale: 100,
				slowStep: 0.01,
				fastStep: 0.1,
				accentColor: "#ef9c56",
			},
		],
	};
	const gridModel: EncoderSectionModel = {
		id: "grid-dynamics-grid",
		label: "Grid",
		description: `${config.columns} columns / ${config.beats} beats = ${config.columns / config.beats} steps per beat`,
		encoders: [
			{
				id: "columns",
				slot: 1,
				target: {
					label: "Columns",
					display: String(config.columns),
					role: "Loop tiles",
				},
				value: config.columns,
				minimum: 4,
				maximum: 32,
				inputScale: 1,
				slowStep: 4,
				fastStep: 4,
				mode: "4 steps",
				accentColor: "#55c7ef",
			},
			{
				id: "beats",
				slot: 2,
				target: {
					label: "Beats / Loop",
					display: String(config.beats),
					role: "Loop duration",
				},
				value: config.beats,
				minimum: 1,
				maximum: 32,
				inputScale: 1,
				slowStep: 1,
				fastStep: 4,
				accentColor: "#55c7ef",
			},
			{
				id: "first-visible",
				slot: 3,
				target: {
					label: "First visible",
					display: `Step ${firstVisible}`,
					role: "Scroll grid",
				},
				value: firstVisible,
				minimum: 1,
				maximum: config.columns,
				inputScale: 1,
				slowStep: 1,
				fastStep: 4,
				accentColor: "#8b9bff",
			},
			{
				id: "zoom",
				slot: 4,
				target: {
					label: "Horizontal zoom",
					display: `${Math.round(zoom * 100)}%`,
					role: "Tile width",
				},
				value: zoom,
				minimum: 0.65,
				maximum: 2,
				inputScale: 100,
				slowStep: 0.05,
				fastStep: 0.2,
				accentColor: "#8b9bff",
			},
			{
				id: "range-start",
				slot: 5,
				target: {
					label: "Range start",
					display: `Step ${rangeStart}`,
					role: "Edit range",
				},
				value: rangeStart,
				minimum: 1,
				maximum: rangeEnd,
				inputScale: 1,
				slowStep: 1,
				fastStep: 4,
				accentColor: "#64d5a3",
			},
			{
				id: "range-end",
				slot: 6,
				target: {
					label: "Range end",
					display: `Step ${rangeEnd}`,
					role: "Edit range",
				},
				value: rangeEnd,
				minimum: rangeStart,
				maximum: config.columns,
				inputScale: 1,
				slowStep: 1,
				fastStep: 4,
				accentColor: "#64d5a3",
			},
		],
	};
	const speedSources = ["Internal", "Speed Group"] as const;
	const speedGroups = ["A", "B", "C", "D", "E"];
	const speedModel: EncoderSectionModel = {
		id: "grid-dynamics-speed",
		label: "Speed",
		description:
			config.speedSource === "speed-group"
				? `Speed Group ${config.speedGroup} · ${config.beats} beats per loop`
				: `${config.internalBpm} BPM internal · ${config.beats} beats per loop`,
		encoders: [
			{
				id: "source",
				slot: 1,
				target: {
					label: "Source",
					display:
						config.speedSource === "internal" ? "Internal" : "Speed Group",
					role: "Clock source",
				},
				value: config.speedSource === "internal" ? 0 : 1,
				minimum: 0,
				maximum: 1,
				inputScale: 1,
				slowStep: 1,
				fastStep: 1,
				touchInteraction: "choices",
				presets: choicePreset(
					config.speedSource === "internal" ? 0 : 1,
					"Clock source",
					speedSources.map((label, index) => ({ value: String(index), label })),
				),
				accentColor: "#55c7ef",
			},
			{
				id: "bpm",
				slot: 2,
				target: {
					label: "Internal BPM",
					display: `${config.internalBpm} BPM`,
					role: "Independent tempo",
				},
				value: config.internalBpm,
				minimum: 1,
				maximum: 999,
				inputScale: 1,
				slowStep: 1,
				fastStep: 5,
				disabled: config.speedSource !== "internal",
				accentColor: "#55c7ef",
			},
			{
				id: "speed-group",
				slot: 3,
				target: {
					label: "Speed Group",
					display: config.speedGroup,
					role: "Desk tempo",
				},
				value: speedGroups.indexOf(config.speedGroup),
				minimum: 0,
				maximum: speedGroups.length - 1,
				inputScale: 1,
				slowStep: 1,
				fastStep: 1,
				disabled: config.speedSource !== "speed-group",
				touchInteraction: "choices",
				presets: choicePreset(
					speedGroups.indexOf(config.speedGroup),
					"Speed Groups",
					speedGroups.map((value, index) => ({
						value: String(index),
						label: `Speed Group ${value}`,
					})),
				),
				accentColor: "#55c7ef",
			},
			{ ...gridModel.encoders[1], id: "speed-beats", slot: 4 },
			{
				id: "transport",
				slot: 5,
				target: {
					label: "Transport",
					display: pendingPlaying
						? "Play queued"
						: playing
							? "Playing"
							: "Stopped",
					role: "Play / Stop",
				},
				value: playing || pendingPlaying ? 1 : 0,
				minimum: 0,
				maximum: 1,
				inputScale: 1,
				slowStep: 1,
				fastStep: 1,
				touchInteraction: "choices",
				presets: choicePreset(playing || pendingPlaying ? 1 : 0, "Transport", [
					{ value: "0", label: "Stop" },
					{ value: "1", label: "Play" },
				]),
				accentColor: playing
					? "#53d88a"
					: pendingPlaying
						? "#f6c453"
						: "#69788a",
			},
			{
				id: "playhead",
				slot: 6,
				target: {
					label: "Playhead",
					display: `Step ${playhead + 1}`,
					role: "Navigate",
				},
				value: playhead,
				minimum: 0,
				maximum: config.columns - 1,
				inputScale: 1,
				slowStep: 1,
				fastStep: 4,
				accentColor: "#ff5d6d",
			},
		],
	};
	const model =
		page === "steps" ? stepsModel : page === "grid" ? gridModel : speedModel;

	const chooseGroup = (index: number) => {
		const next = groups[clamp(Math.round(index), 0, groups.length - 1)];
		if (!next) return;
		onSelection({
			groupId: next.id,
			laneId: next.lanes[0]?.id ?? "",
			column: selection.column,
		});
	};
	const chooseLane = (index: number) => {
		const next =
			group?.lanes[clamp(Math.round(index), 0, (group?.lanes.length ?? 1) - 1)];
		if (next) onSelection({ ...selection, laneId: next.id });
	};
	const updateCell = (key: "attack" | "decay", normalized: number) => {
		if (!group || !lane) return;
		onGroups(
			groups.map((candidate) =>
				candidate.id !== group.id
					? candidate
					: {
							...candidate,
							lanes: candidate.lanes.map((candidateLane) => {
								if (candidateLane.id !== lane.id) return candidateLane;
								const cells = [...candidateLane.cells];
								cells[selection.column] = {
									...cells[selection.column],
									[key]: Math.round(clamp(normalized, 0, 1) * 100),
								};
								return { ...candidateLane, cells };
							}),
						},
			),
		);
	};
	const resizeGrid = (columns: number) => {
		const nextColumns = clamp(Math.round(columns / 4) * 4, 4, 32);
		onConfig({ ...config, columns: nextColumns });
		onGroups(
			groups.map((candidate) => ({
				...candidate,
				lanes: candidate.lanes.map((candidateLane) => ({
					...candidateLane,
					cells: Array.from(
						{ length: nextColumns },
						(_, index) =>
							candidateLane.cells[index] ?? {
								preset: null,
								alternate: null,
								attack: 0,
								decay: 0,
							},
					),
				})),
			})),
		);
		onSelection({
			...selection,
			column: Math.min(selection.column, nextColumns - 1),
		});
		onPlayhead(Math.min(playhead, nextColumns - 1));
		setRangeEnd(nextColumns);
	};
	const apply = (id: string, value: number) => {
		if (id === "group") chooseGroup(value);
		if (id === "lane") chooseLane(value);
		if (id === "step")
			onSelection({
				...selection,
				column: clamp(Math.round(value), 0, config.columns - 1),
			});
		if (id === "preset")
			onActivePreset(
				compatiblePresets[
					clamp(Math.round(value), 0, compatiblePresets.length - 1)
				]?.id ?? activePreset,
			);
		if (id === "attack") updateCell("attack", value);
		if (id === "decay") updateCell("decay", value);
		if (id === "columns") resizeGrid(value);
		if (id === "beats" || id === "speed-beats")
			onConfig({ ...config, beats: clamp(Math.round(value), 1, 32) });
		if (id === "first-visible")
			setFirstVisible(clamp(Math.round(value), 1, config.columns));
		if (id === "zoom") setZoom(clamp(value, 0.65, 2));
		if (id === "range-start")
			setRangeStart(clamp(Math.round(value), 1, rangeEnd));
		if (id === "range-end")
			setRangeEnd(clamp(Math.round(value), rangeStart, config.columns));
		if (id === "source")
			onConfig({
				...config,
				speedSource: Math.round(value) === 0 ? "internal" : "speed-group",
			});
		if (id === "bpm")
			onConfig({ ...config, internalBpm: clamp(Math.round(value), 1, 999) });
		if (id === "speed-group")
			onConfig({
				...config,
				speedGroup:
					speedGroups[clamp(Math.round(value), 0, speedGroups.length - 1)] ??
					"A",
			});
		if (id === "transport") onPlaying(Math.round(value) > 0);
		if (id === "playhead")
			onPlayhead(clamp(Math.round(value), 0, config.columns - 1));
	};

	return (
		<div className="parameter-controls">
			<div className="family-tabs grid-dynamics-encoder-tabs">
				<Button active={page === "steps"} onClick={() => setPage("steps")}>
					Steps
				</Button>
				<Button active={page === "grid"} onClick={() => setPage("grid")}>
					Grid
				</Button>
				<Button active={page === "speed"} onClick={() => setPage("speed")}>
					Speed
				</Button>
				<span>{model.description}</span>
			</div>
			<div className="parameter-surfaces">
				<EncoderSection
					className="grid-dynamics-encoder-section"
					model={model}
					surface={hardware ? "hardware" : "touch"}
					showHeader={false}
					callbacks={{
						onRelativeChange: (id, delta) => {
							const encoder = model.encoders.find(
								(candidate) => candidate.id === id,
							);
							if (encoder) apply(id, encoder.value + delta);
						},
						onAbsoluteChange: apply,
						onPresetSelect: (id, value) => apply(id, Number(value)),
					}}
				/>
			</div>
		</div>
	);
}

function FullApplicationGridDynamics({
	hardware,
	preload = false,
	marketing = false,
}: {
	hardware: boolean;
	preload?: boolean;
	marketing?: boolean;
}) {
	const [config, setConfig] = useState<GridDynamicConfig>({
		columns: 16,
		beats: 4,
		speedSource: "speed-group",
		speedGroup: "A",
		internalBpm: 120,
	});
	const [groups, setGroups] = useState(() => seededGroups(config.columns));
	const [activePreset, setActivePreset] = useState("intensity-full");
	const [selection, setSelection] = useState<GridCellSelection>({
		groupId: "front-wash",
		laneId: "front-wash-intensity",
		column: 0,
	});
	const [playing, setPlaying] = useState(true);
	const [pendingPlaying, setPendingPlaying] = useState(false);
	const [playhead, setPlayhead] = useState(5);
	const setTransport = (next: boolean) => {
		if (!next) {
			setPlaying(false);
			setPendingPlaying(false);
			return;
		}
		if (preload) {
			setPendingPlaying(true);
			return;
		}
		setPlaying(true);
	};
	const programmer = useMemo(
		() => (
			<GridDynamicsEncoderDeck
				hardware={hardware}
				groups={groups}
				config={config}
				activePreset={activePreset}
				selection={selection}
				playing={playing}
				pendingPlaying={pendingPlaying}
				playhead={playhead}
				onGroups={setGroups}
				onConfig={setConfig}
				onActivePreset={setActivePreset}
				onSelection={setSelection}
				onPlaying={setTransport}
				onPlayhead={setPlayhead}
			/>
		),
		[
			activePreset,
			config,
			groups,
			hardware,
			pendingPlaying,
			playhead,
			playing,
			selection,
		],
	);
	return (
		<ApplicationStateHarness
			actions={hardware ? [{ type: "SET_MIDI_PROFILE", value: true }] : []}
		>
			<AppShellView
				dock={
					<LeftDock
						presentation={{
							showIdentity: marketing ? "Demo Show" : "Grid Dynamics UI Review",
							showIndicator: {
								label: marketing ? "Demo show" : "Offline mock",
								detail: marketing
									? "Deterministic Grid Dynamic presentation."
									: "All changes stay in Storybook memory",
								className: marketing
									? "show-status-connected"
									: "show-status-warning",
								connected: marketing,
							},
							clock: <Clock now={new Date(2026, 6, 30, 21, 16, 32)} />,
						}}
					/>
				}
				workspace={
					<GridDesktop id="grid-dynamics-review" name="Grid Dynamics Review">
						<PaneView
							maximized
							showHeader={false}
							pane={{
								id: "grid-dynamics",
								title: "Grid Dynamics",
								type: "dynamics",
								x: 1,
								y: 1,
								width: 24,
								height: 18,
							}}
						>
							<GridDynamicsWindow
								name="Neon Chase"
								groups={groups}
								config={config}
								activePreset={activePreset}
								selection={selection}
								playing={playing}
								preload={preload}
								pendingPlaying={pendingPlaying}
								playhead={playhead}
								onGroups={setGroups}
								onConfig={setConfig}
								onActivePreset={setActivePreset}
								onSelection={setSelection}
								onPlaying={setTransport}
								onPlayhead={setPlayhead}
							/>
						</PaneView>
					</GridDesktop>
				}
				control={
					<CommandSectionFixture
						inheritAppState
						initialMode="programmer"
						hardware={hardware}
						preloadArmed={preload}
						programmer={programmer}
					/>
				}
			/>
		</ApplicationStateHarness>
	);
}

export function MarketingGridDynamicsApplication() {
	return <FullApplicationGridDynamics hardware={false} marketing />;
}

export const FullApplicationDiscussion: Story = {
	render: (_args, context) => (
		<FullApplicationGridDynamics
			hardware={context.globals.mode === "hardware"}
		/>
	),
};

export const PreloadPlayQueued: Story = {
	render: (_args, context) => (
		<FullApplicationGridDynamics
			hardware={context.globals.mode === "hardware"}
			preload
		/>
	),
};
