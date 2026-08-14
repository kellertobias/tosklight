/**
 * Future Grid Dynamics feature prototype.
 *
 * This window is intentionally Storybook-first and has no runtime, persistence,
 * programmer, or output wiring. Keep it as the product-design contract for the
 * proposed drum-machine-style Dynamic editor.
 */
import {
	Button,
	CheckboxField,
	ModalFrame,
	NumberField,
	SelectField,
	WindowHeader,
	WindowScrollArea,
} from "@tosklight/ui";
import { type CSSProperties, useEffect, useRef, useState } from "react";
import "./GridDynamicsWindow.css";

export type GridLaneKind = "intensity" | "position" | "color";
export type GridSpeedSource = "internal" | "speed-group";

export interface GridDynamicCell {
	preset: string | null;
	alternate: string | null;
	attack: number;
	decay: number;
}

export interface GridDynamicLane {
	id: string;
	kind: GridLaneKind;
	label: string;
	cells: GridDynamicCell[];
}

export interface GridDynamicGroup {
	id: string;
	number: number;
	name: string;
	fixtureCount: number;
	color: string;
	lanes: GridDynamicLane[];
}

export interface GridDynamicConfig {
	columns: number;
	beats: number;
	speedSource: GridSpeedSource;
	speedGroup: string;
	internalBpm: number;
}

export interface GridCellSelection {
	groupId: string;
	laneId: string;
	column: number;
}

export interface GridPreset {
	id: string;
	label: string;
	kind: GridLaneKind;
	color: string;
}

export const gridPresets: GridPreset[] = [
	{ id: "intensity-full", label: "Full", kind: "intensity", color: "#f6c453" },
	{ id: "intensity-50", label: "50%", kind: "intensity", color: "#b9892e" },
	{
		id: "intensity-blackout",
		label: "Blackout",
		kind: "intensity",
		color: "#59616d",
	},
	{ id: "position-fan", label: "Fan Out", kind: "position", color: "#55c7ef" },
	{ id: "position-cross", label: "Cross", kind: "position", color: "#8b9bff" },
	{
		id: "position-audience",
		label: "Audience",
		kind: "position",
		color: "#b779ff",
	},
	{ id: "color-magenta", label: "Magenta", kind: "color", color: "#f044c8" },
	{ id: "color-cyan", label: "Cyan", kind: "color", color: "#23cdea" },
	{ id: "color-amber", label: "Amber", kind: "color", color: "#ff9f2f" },
	{ id: "color-blue", label: "Deep Blue", kind: "color", color: "#4267ff" },
];

export interface GridDynamicsWindowProps {
	name: string;
	groups: GridDynamicGroup[];
	config: GridDynamicConfig;
	activePreset: string;
	selection: GridCellSelection;
	playing: boolean;
	preload: boolean;
	pendingPlaying: boolean;
	playhead: number;
	onGroups(groups: GridDynamicGroup[]): void;
	onConfig(config: GridDynamicConfig): void;
	onActivePreset(presetId: string): void;
	onSelection(selection: GridCellSelection): void;
	onPlaying(playing: boolean): void;
	onPlayhead(column: number): void;
}

function presetById(id: string | null) {
	return gridPresets.find((preset) => preset.id === id) ?? null;
}

function laneAccent(kind: GridLaneKind) {
	if (kind === "intensity") return "#f6c453";
	if (kind === "position") return "#55c7ef";
	return "#ef5ac8";
}

function resizeCells(cells: GridDynamicCell[], columns: number) {
	return Array.from({ length: columns }, (_, index) =>
		cells[index]
			? { ...cells[index] }
			: { preset: null, alternate: null, attack: 0, decay: 0 },
	);
}

function Tile({
	cell,
	column,
	lane,
	selected,
	playhead,
	onChoose,
	onPaint,
	onPresetPicker,
}: {
	cell: GridDynamicCell;
	column: number;
	lane: GridDynamicLane;
	selected: boolean;
	playhead: boolean;
	onChoose(): void;
	onPaint(): void;
	onPresetPicker(): void;
}) {
	const holdTimer = useRef<number | null>(null);
	const held = useRef(false);
	const clearHold = () => {
		if (holdTimer.current !== null) window.clearTimeout(holdTimer.current);
		holdTimer.current = null;
	};
	useEffect(() => clearHold, []);
	const preset = presetById(cell.preset);
	return (
		<Button
			type="button"
			className={`grid-dynamic-tile ${selected ? "is-selected" : ""} ${playhead ? "is-playhead" : ""} ${preset ? "has-preset" : ""}`}
			style={
				preset
					? ({ "--tile-accent": preset.color } as CSSProperties)
					: undefined
			}
			aria-label={`${lane.label}, step ${column + 1}: ${preset?.label ?? "Current"}`}
			title={`${preset?.label ?? "Current"} · Attack ${cell.attack}% · Decay ${cell.decay}%`}
			onContextMenu={(event) => {
				event.preventDefault();
				onChoose();
				onPresetPicker();
			}}
			onPointerDown={(_event) => {
				held.current = false;
				holdTimer.current = window.setTimeout(() => {
					held.current = true;
					onChoose();
					onPresetPicker();
				}, 520);
			}}
			onPointerEnter={(event) => {
				if (event.buttons !== 1 || holdTimer.current !== null) return;
				onChoose();
				onPaint();
			}}
			onPointerUp={() => {
				clearHold();
				if (held.current) {
					held.current = false;
					return;
				}
				onChoose();
				onPaint();
			}}
			onPointerCancel={clearHold}
		>
			<span>{preset?.label ?? "Current"}</span>
			{(cell.attack > 0 || cell.decay > 0) && (
				<small>
					{cell.attack > 0 ? `A${cell.attack}` : ""}
					{cell.attack > 0 && cell.decay > 0 ? " · " : ""}
					{cell.decay > 0 ? `D${cell.decay}` : ""}
				</small>
			)}
		</Button>
	);
}

function GroupRows({
	group,
	config,
	selection,
	playhead,
	onSelection,
	onPaint,
	onPresetPicker,
	onAddLane,
}: {
	group: GridDynamicGroup;
	config: GridDynamicConfig;
	selection: GridCellSelection;
	playhead: number;
	onSelection(selection: GridCellSelection): void;
	onPaint(selection: GridCellSelection): void;
	onPresetPicker(selection: GridCellSelection): void;
	onAddLane(groupId: string): void;
}) {
	return (
		<section
			className="grid-dynamic-group"
			style={{ "--group-color": group.color } as CSSProperties}
		>
			<header className="grid-dynamic-group-heading">
				<span>
					<strong>
						{group.number} · {group.name}
					</strong>
					<small>
						{group.fixtureCount} fixtures · {group.lanes.length} lanes
					</small>
				</span>
				<Button size="compact" onClick={() => onAddLane(group.id)}>
					Add lane
				</Button>
			</header>
			<div className="grid-dynamic-group-lanes">
				{group.lanes.map((lane) => (
					<div className="grid-dynamic-lane" key={lane.id}>
						<div className="grid-dynamic-lane-name">
							<i style={{ background: laneAccent(lane.kind) }} />
							<span>
								<strong>{lane.label}</strong>
								<small>{lane.kind}</small>
							</span>
						</div>
						<div
							className="grid-dynamic-tiles"
							style={{
								gridTemplateColumns: `repeat(${config.columns}, minmax(2.75rem, 1fr))`,
							}}
						>
							{lane.cells.slice(0, config.columns).map((cell, column) => {
								const cellSelection = {
									groupId: group.id,
									laneId: lane.id,
									column,
								};
								return (
									<Tile
										key={`${lane.id}-${column}`}
										cell={cell}
										column={column}
										lane={lane}
										selected={
											selection.groupId === group.id &&
											selection.laneId === lane.id &&
											selection.column === column
										}
										playhead={playhead === column}
										onChoose={() => onSelection(cellSelection)}
										onPaint={() => onPaint(cellSelection)}
										onPresetPicker={() => onPresetPicker(cellSelection)}
									/>
								);
							})}
						</div>
					</div>
				))}
			</div>
		</section>
	);
}

function GridDynamicsHeader({
	props,
	setModal,
}: {
	props: GridDynamicsWindowProps;
	setModal(value: "groups" | "speed" | "grid" | "preset" | "lane" | null): void;
}) {
	const columnsPerBeat = props.config.columns / props.config.beats;
	return (
		<WindowHeader
			title={`Grid Dynamic · ${props.name}`}
			info={{
				primary: `${props.config.columns} steps · ${props.config.beats} beats`,
				secondary: `${columnsPerBeat} steps / beat · ${
					props.config.speedSource === "speed-group"
						? `Speed Group ${props.config.speedGroup}`
						: `${props.config.internalBpm} BPM internal`
				}${
					props.pendingPlaying
						? " · Preloaded · Play queued"
						: props.preload
							? " · Preload armed"
							: ""
				}`,
			}}
			groups={[
				{ id: "grid-configuration", actions: [
					{
						id: "groups",
						label: "Select Groups",
						onPress: () => setModal("groups"),
					},
					{
						id: "speed",
						label: "Configure Speed Group",
						onPress: () => setModal("speed"),
					},
					{
						id: "grid",
						label: "Configure Grid",
						onPress: () => setModal("grid"),
					},
				] },
				{ id: "grid-transport", actions: [
					{
						id: "transport",
						label: props.playing || props.pendingPlaying ? "Stop" : "Play",
						active: props.playing || props.pendingPlaying,
						onPress: () =>
							props.onPlaying(!(props.playing || props.pendingPlaying)),
					},
				] },
			]}
		/>
	);
}

function GridDynamicsRuler({ props }: { props: GridDynamicsWindowProps }) {
	const columnsPerBeat = props.config.columns / props.config.beats;
	return (
		<div className="grid-dynamic-ruler-shell">
			<div className="grid-dynamic-ruler-label">
				<strong>Beat grid</strong>
				<small>Long-press a tile to choose the preset brush</small>
			</div>
			<div
				className="grid-dynamic-ruler"
				style={{
					gridTemplateColumns: `repeat(${props.config.columns}, minmax(2.75rem, 1fr))`,
				}}
			>
				{Array.from({ length: props.config.columns }, (_, column) => {
					const beat = column / columnsPerBeat;
					const beatStart = Number.isInteger(beat);
					return (
						<Button
							type="button"
							key={column}
							className={`${beatStart ? "is-beat" : ""} ${props.playhead === column ? "is-playhead" : ""}`}
							onClick={() => props.onPlayhead(column)}
						>
							<strong>{column + 1}</strong>
							<small>{beatStart ? `Beat ${beat + 1}` : "·"}</small>
						</Button>
					);
				})}
			</div>
		</div>
	);
}

function GridDynamicsStatus({
	props,
	selectedLane,
	selectedCell,
}: {
	props: GridDynamicsWindowProps;
	selectedLane: GridDynamicLane | undefined;
	selectedCell: GridDynamicCell | undefined;
}) {
	return (
		<footer className="grid-dynamic-status">
			<span className={props.playing ? "is-running" : ""}>
				<i />{" "}
				{props.pendingPlaying
					? "Preloaded · Play queued"
					: props.playing
						? "Playing live"
						: "Stopped"}
			</span>
			<span>
				Brush <b>{presetById(props.activePreset)?.label}</b>
			</span>
			<span>
				Selected{" "}
				<b>
					{selectedLane?.label ?? "—"} · Step {props.selection.column + 1}
				</b>
			</span>
			<span>
				Attack <b>{selectedCell?.attack ?? 0}%</b> · Decay{" "}
				<b>{selectedCell?.decay ?? 0}%</b>
			</span>
		</footer>
	);
}

function selectGridGroups(props: GridDynamicsWindowProps, ids: string[]) {
	return initialGroupChoices
		.filter((choice) => ids.includes(choice.id))
		.map((choice) => {
			const existing = props.groups.find((group) => group.id === choice.id);
			return existing ?? createGridGroup(choice, props.config.columns);
		});
}

function paintGridCell(
	props: GridDynamicsWindowProps,
	selection: GridCellSelection,
	forcedPreset?: string,
) {
	const group = props.groups.find(
		(candidate) => candidate.id === selection.groupId,
	);
	const lane = group?.lanes.find(
		(candidate) => candidate.id === selection.laneId,
	);
	if (!group || !lane) return;
	const brush = presetById(forcedPreset ?? props.activePreset);
	if (!brush || brush.kind !== lane.kind) return;
	props.onGroups(
		props.groups.map((candidate) =>
			candidate.id !== group.id
				? candidate
				: {
						...candidate,
						lanes: candidate.lanes.map((candidateLane) => {
							if (candidateLane.id !== lane.id) return candidateLane;
							const cells = [...candidateLane.cells];
							const current = cells[selection.column];
							cells[selection.column] =
								current.preset === brush.id
									? {
											...current,
											preset: current.alternate,
											alternate: current.preset,
										}
									: { ...current, preset: brush.id, alternate: current.preset };
							return { ...candidateLane, cells };
						}),
					},
		),
	);
}

export function GridDynamicsWindow(props: GridDynamicsWindowProps) {
	const [modal, setModal] = useState<
		"groups" | "speed" | "grid" | "preset" | "lane" | null
	>(null);
	const [laneGroupId, setLaneGroupId] = useState<string | null>(null);
	const selectedLane = props.groups
		.find((group) => group.id === props.selection.groupId)
		?.lanes.find((lane) => lane.id === props.selection.laneId);
	const selectedCell = selectedLane?.cells[props.selection.column];

	const paint = (selection: GridCellSelection, forcedPreset?: string) =>
		paintGridCell(props, selection, forcedPreset);

	return (
		<section className="grid-dynamics-window">
			<GridDynamicsHeader props={props} setModal={setModal} />
			<GridDynamicsRuler props={props} />
			<div className="grid-dynamic-workspace">
				<WindowScrollArea>
					<div className="grid-dynamic-groups">
						{props.groups.map((group) => (
							<GroupRows
								key={group.id}
								group={group}
								config={props.config}
								selection={props.selection}
								playhead={props.playhead}
								onSelection={props.onSelection}
								onPaint={paint}
								onPresetPicker={(selection) => {
									props.onSelection(selection);
									setModal("preset");
								}}
								onAddLane={(groupId) => {
									setLaneGroupId(groupId);
									setModal("lane");
								}}
							/>
						))}
					</div>
				</WindowScrollArea>
			</div>
			<GridDynamicsStatus
				props={props}
				selectedLane={selectedLane}
				selectedCell={selectedCell}
			/>
			{modal === "groups" && (
				<GroupSelectionModal
					selected={props.groups.map((group) => group.id)}
					onSave={(ids) => {
						props.onGroups(selectGridGroups(props, ids));
						setModal(null);
					}}
					onClose={() => setModal(null)}
				/>
			)}
			{modal === "speed" && (
				<SpeedModal
					config={props.config}
					onSave={(config) => {
						props.onConfig(config);
						setModal(null);
					}}
					onClose={() => setModal(null)}
				/>
			)}
			{modal === "grid" && (
				<GridModal
					config={props.config}
					onSave={(config) => {
						props.onConfig(config);
						props.onGroups(
							props.groups.map((group) => ({
								...group,
								lanes: group.lanes.map((lane) => ({
									...lane,
									cells: resizeCells(lane.cells, config.columns),
								})),
							})),
						);
						props.onPlayhead(Math.min(props.playhead, config.columns - 1));
						setModal(null);
					}}
					onClose={() => setModal(null)}
				/>
			)}
			{modal === "preset" && selectedLane && (
				<PresetModal
					kind={selectedLane.kind}
					selected={props.activePreset}
					onChoose={(preset) => {
						props.onActivePreset(preset);
						paint(props.selection, preset);
						setModal(null);
					}}
					onClose={() => setModal(null)}
				/>
			)}
			{modal === "lane" && laneGroupId && (
				<LaneModal
					group={props.groups.find((group) => group.id === laneGroupId)}
					columns={props.config.columns}
					onAdd={(lane) => {
						props.onGroups(
							props.groups.map((group) =>
								group.id === laneGroupId
									? { ...group, lanes: [...group.lanes, lane] }
									: group,
							),
						);
						setModal(null);
					}}
					onClose={() => setModal(null)}
				/>
			)}
		</section>
	);
}

export const initialGroupChoices = [
	{
		id: "front-wash",
		number: 1,
		name: "Front Wash",
		fixtureCount: 8,
		color: "#59baf1",
	},
	{
		id: "moving-heads",
		number: 4,
		name: "Moving Heads",
		fixtureCount: 12,
		color: "#b875ef",
	},
	{
		id: "led-lines",
		number: 7,
		name: "LED Lines",
		fixtureCount: 16,
		color: "#ed5dac",
	},
	{
		id: "audience",
		number: 12,
		name: "Audience Blinders",
		fixtureCount: 6,
		color: "#efaa4c",
	},
] as const;

function emptyCells(columns: number) {
	return Array.from({ length: columns }, () => ({
		preset: null,
		alternate: null,
		attack: 0,
		decay: 0,
	}));
}

export function createGridGroup(
	choice: (typeof initialGroupChoices)[number],
	columns: number,
): GridDynamicGroup {
	return {
		...choice,
		lanes: [
			{
				id: `${choice.id}-intensity`,
				kind: "intensity",
				label: "Intensity",
				cells: emptyCells(columns),
			},
			{
				id: `${choice.id}-color`,
				kind: "color",
				label: "Color",
				cells: emptyCells(columns),
			},
		],
	};
}

function GroupSelectionModal({
	selected,
	onSave,
	onClose,
}: {
	selected: string[];
	onSave(ids: string[]): void;
	onClose(): void;
}) {
	const [ids, setIds] = useState(selected);
	return (
		<ModalFrame
			id="grid-dynamic-groups"
			ariaLabel="Select Grid Dynamic groups"
			title="Select Groups"
			details="Each group starts with one Intensity lane and one Color lane"
			accept={{
				id: "apply",
				label: "Apply groups",
				variant: "primary",
				onPress: () => onSave(ids),
			}}
			onClose={onClose}
		>
			<div className="grid-dynamic-modal-list">
				{initialGroupChoices.map((group) => (
					<CheckboxField
						key={group.id}
						label={`${group.number} · ${group.name}`}
						description={`${group.fixtureCount} fixtures`}
						checked={ids.includes(group.id)}
						stateLabel={ids.includes(group.id) ? "Included" : "Excluded"}
						onChange={(event) =>
							setIds((current) =>
								event.target.checked
									? [...current, group.id]
									: current.filter((id) => id !== group.id),
							)
						}
					/>
				))}
			</div>
		</ModalFrame>
	);
}

function SpeedModal({
	config,
	onSave,
	onClose,
}: {
	config: GridDynamicConfig;
	onSave(config: GridDynamicConfig): void;
	onClose(): void;
}) {
	const [draft, setDraft] = useState(config);
	return (
		<ModalFrame
			id="grid-dynamic-speed"
			ariaLabel="Configure Grid Dynamic speed"
			title="Configure Speed Group"
			details="Run from the desk clock or keep an independent internal tempo"
			accept={{
				id: "apply",
				label: "Apply speed",
				variant: "primary",
				onPress: () => onSave(draft),
			}}
			onClose={onClose}
		>
			<div className="grid-dynamic-modal-form">
				<SelectField
					label="Clock source"
					value={draft.speedSource}
					options={[
						{ value: "speed-group", label: "Speed Group" },
						{ value: "internal", label: "Internal clock" },
					]}
					onChange={(value) =>
						setDraft({ ...draft, speedSource: value as GridSpeedSource })
					}
				/>
				{draft.speedSource === "speed-group" ? (
					<SelectField
						label="Speed group"
						value={draft.speedGroup}
						options={["A", "B", "C", "D", "E"].map((value) => ({
							value,
							label: `Speed Group ${value}`,
						}))}
						onChange={(value) => setDraft({ ...draft, speedGroup: value })}
					/>
				) : (
					<NumberField
						label="Internal tempo"
						value={draft.internalBpm}
						min={1}
						max={999}
						unit="BPM"
						onChange={(event) =>
							setDraft({
								...draft,
								internalBpm: Number(event.target.value) || 1,
							})
						}
					/>
				)}
			</div>
		</ModalFrame>
	);
}

function GridModal({
	config,
	onSave,
	onClose,
}: {
	config: GridDynamicConfig;
	onSave(config: GridDynamicConfig): void;
	onClose(): void;
}) {
	const [draft, setDraft] = useState(config);
	return (
		<ModalFrame
			id="grid-dynamic-grid"
			ariaLabel="Configure Grid Dynamic grid"
			title="Configure Grid"
			details="Choose the number of tiles and how many beats the complete loop spans"
			accept={{
				id: "apply",
				label: "Apply grid",
				variant: "primary",
				onPress: () => onSave(draft),
			}}
			onClose={onClose}
		>
			<div className="grid-dynamic-modal-form">
				<NumberField
					label="Columns"
					value={draft.columns}
					min={4}
					max={32}
					step={4}
					onChange={(event) =>
						setDraft({
							...draft,
							columns: Math.max(
								4,
								Math.round((Number(event.target.value) || 4) / 4) * 4,
							),
						})
					}
				/>
				<NumberField
					label="Beats per loop"
					value={draft.beats}
					min={1}
					max={32}
					onChange={(event) =>
						setDraft({
							...draft,
							beats: Math.max(1, Math.round(Number(event.target.value) || 1)),
						})
					}
				/>
				<p className="grid-dynamic-grid-summary">
					<strong>{draft.columns / draft.beats} steps per beat</strong>
					<span>
						{draft.columns === 16 && draft.beats === 4
							? "16ths"
							: draft.columns === 16 && draft.beats === 8
								? "8ths"
								: draft.columns === 16 && draft.beats === 16
									? "4ths"
									: "Custom subdivision"}
					</span>
				</p>
			</div>
		</ModalFrame>
	);
}

function PresetModal({
	kind,
	selected,
	onChoose,
	onClose,
}: {
	kind: GridLaneKind;
	selected: string;
	onChoose(id: string): void;
	onClose(): void;
}) {
	return (
		<ModalFrame
			id="grid-dynamic-preset"
			ariaLabel={`Choose ${kind} preset`}
			title="Select Preset"
			details="This becomes the brush for every compatible tile you click or drag across"
			onClose={onClose}
		>
			<div className="grid-dynamic-preset-grid">
				{gridPresets
					.filter((preset) => preset.kind === kind)
					.map((preset) => (
						<Button
							key={preset.id}
							active={selected === preset.id}
							className="grid-dynamic-preset-button"
							style={{ "--preset-color": preset.color } as CSSProperties}
							onClick={() => onChoose(preset.id)}
						>
							<i />
							<span>{preset.label}</span>
						</Button>
					))}
			</div>
		</ModalFrame>
	);
}

function LaneModal({
	group,
	columns,
	onAdd,
	onClose,
}: {
	group: GridDynamicGroup | undefined;
	columns: number;
	onAdd(lane: GridDynamicLane): void;
	onClose(): void;
}) {
	const [kind, setKind] = useState<GridLaneKind>("position");
	const count =
		(group?.lanes.filter((lane) => lane.kind === kind).length ?? 0) + 1;
	return (
		<ModalFrame
			id="grid-dynamic-lane"
			ariaLabel="Add Grid Dynamic lane"
			title="Add lane"
			details={group ? `${group.number} · ${group.name}` : undefined}
			accept={{
				id: "add",
				label: "Add lane",
				variant: "primary",
				onPress: () =>
					onAdd({
						id: `${group?.id}-${kind}-${Date.now()}`,
						kind,
						label:
							count === 1
								? kind[0].toUpperCase() + kind.slice(1)
								: `${kind[0].toUpperCase() + kind.slice(1)} ${count}`,
						cells: emptyCells(columns),
					}),
			}}
			onClose={onClose}
		>
			<div className="grid-dynamic-modal-form">
				<SelectField
					label="Attribute family"
					value={kind}
					options={[
						{ value: "intensity", label: "Intensity" },
						{ value: "position", label: "Position" },
						{ value: "color", label: "Color" },
					]}
					onChange={(value) => setKind(value as GridLaneKind)}
				/>
				<p>Groups may contain multiple lanes of the same family.</p>
			</div>
		</ModalFrame>
	);
}
