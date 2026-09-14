import { Button, InputModal, SwitchField } from "@tosklight/ui";
import { useState } from "react";
import type { AttributeDescriptor, FixtureChannel, FixtureMode } from "../wire";
import { maxRaw } from "../sheet/fixtureProfileModel";
import { AttributePickerModal, STATIC_ATTRIBUTE } from "./attributePicker";
import { channelLabel, channelUnit, mappingSummary } from "./channelLabels";
import { applyCanonicalChannelAttribute } from "./channelModel";
import { replaceChannel } from "./channelOperations";
import {
	CHANNEL_LEVELS,
	type ChannelLevel,
	type SlotRow,
	moveSlotRow,
	moveSlotRowTo,
	removeSlotRow,
	setRowLevel,
	slotRowKey,
	slotRows,
} from "./channelSlots";
import { ConfirmDialog } from "./dialogs";
import { MastersModal, mastersSummary } from "./mastersPicker";
import { OptionPickerModal } from "./optionPicker";
import { TrashIcon } from "./trashIcon";

type RawField = "default_raw" | "highlight_raw";

/** Which of a row's settings is open in a window of its own. */
type SlotEdit =
	| { kind: "attribute"; channelId: string }
	| { kind: "head"; channelId: string }
	| { kind: "level"; rowKey: string }
	| { kind: "raw"; channelId: string; field: RawField }
	| { kind: "masters"; channelId: string };

const COLUMNS = [
	"Slot",
	"Head",
	"Attribute",
	"Level",
	"Default",
	"Highlight",
	"Mapping",
	"Invert",
	"Snap",
	"Masters",
	"Order",
];

function TouchSlotDragHandle({
	rowKey,
	onMove,
}: {
	rowKey: string;
	onMove: (sourceKey: string, targetKey: string) => void;
}) {
	return (
		<span
			className="drag-handle touch-drag-handle"
			aria-hidden="true"
			title="Drag to move this slot"
			onPointerDown={(event) => {
				if (event.pointerType === "mouse") return;
				event.preventDefault();
				event.currentTarget.setPointerCapture(event.pointerId);
			}}
			onPointerMove={(event) => {
				if (
					event.pointerType === "mouse" ||
					!event.currentTarget.hasPointerCapture(event.pointerId)
				)
					return;
				const target = document
					.elementFromPoint(event.clientX, event.clientY)
					?.closest<HTMLElement>("[data-slot-row-key]")?.dataset.slotRowKey;
				if (target && target !== rowKey) onMove(rowKey, target);
			}}
			onPointerUp={(event) =>
				event.currentTarget.hasPointerCapture(event.pointerId) &&
				event.currentTarget.releasePointerCapture(event.pointerId)
			}
		>
			⠿
		</span>
	);
}

/** A cell that shows its value and opens a window to change it. */
function ValueCell({
	label,
	children,
	onPress,
}: {
	label: string;
	children: React.ReactNode;
	onPress: () => void;
}) {
	return (
		<Button className="fixture-cell-value" aria-label={label} onClick={onPress}>
			{children}
		</Button>
	);
}

/** The cells only a Coarse row fills; a further byte shows a dash that points back to it. */
function ChannelSettingCells({
	row,
	name,
	attributeRegistry,
	onChannel,
	onEdit,
	onEditMapping,
}: {
	row: SlotRow;
	name: string;
	attributeRegistry: AttributeDescriptor[];
	onChannel: (channel: FixtureChannel) => void;
	onEdit: (edit: SlotEdit) => void;
	onEditMapping: (channel: FixtureChannel) => void;
}) {
	const { channel } = row;
	if (row.level > 0)
		return (
			<>
				{COLUMNS.slice(4, 10).map((column) => (
					<td key={column}>
						<span className="fixture-slot-inherited" title="Set on the Coarse slot">
							—
						</span>
					</td>
				))}
			</>
		);
	const toggle = (key: "invert" | "snap", label: string) => (
		<SwitchField
			controlOnly
			label={`${label} ${name}`}
			aria-label={`${label} ${name}`}
			checked={channel[key]}
			onChange={(event) => onChannel({ ...channel, [key]: event.target.checked })}
		/>
	);
	return (
		<>
			<td>
				<ValueCell
					label={`Default for ${name}`}
					onPress={() =>
						onEdit({ kind: "raw", channelId: channel.id, field: "default_raw" })
					}
				>
					{channel.default_raw}
				</ValueCell>
			</td>
			<td>
				<ValueCell
					label={`Highlight for ${name}`}
					onPress={() =>
						onEdit({ kind: "raw", channelId: channel.id, field: "highlight_raw" })
					}
				>
					{channel.highlight_raw}
				</ValueCell>
			</td>
			<td>
				<ValueCell
					label={`Edit ${channel.attribute} mapping`}
					onPress={() => onEditMapping(channel)}
				>
					{mappingSummary(channel, channelUnit(channel, attributeRegistry))}
				</ValueCell>
			</td>
			<td className="fixture-cell-switch">{toggle("invert", "Invert")}</td>
			<td className="fixture-cell-switch">{toggle("snap", "Snap")}</td>
			<td>
				<ValueCell
					label={`Masters for ${name}`}
					onPress={() => onEdit({ kind: "masters", channelId: channel.id })}
				>
					{mastersSummary(channel)}
				</ValueCell>
			</td>
		</>
	);
}

function SlotOrderActions({
	name,
	first,
	last,
	onMove,
	onRemove,
}: {
	name: string;
	first: boolean;
	last: boolean;
	onMove: (direction: -1 | 1) => void;
	onRemove: () => void;
}) {
	return (
		<div className="reorder-actions">
			<Button
				iconOnly
				aria-label={`Move ${name} up`}
				disabled={first}
				onClick={() => onMove(-1)}
			>
				▲
			</Button>
			<Button
				iconOnly
				aria-label={`Move ${name} down`}
				disabled={last}
				onClick={() => onMove(1)}
			>
				▼
			</Button>
			<Button
				iconOnly
				variant="danger"
				className="fixture-trash-button"
				aria-label={`Remove ${name}`}
				onClick={onRemove}
			>
				<TrashIcon />
			</Button>
		</div>
	);
}

type RowProps = {
	mode: FixtureMode;
	row: SlotRow;
	index: number;
	count: number;
	attributeRegistry: AttributeDescriptor[];
	onChannel: (channel: FixtureChannel) => void;
	onEdit: (edit: SlotEdit) => void;
	onEditMapping: (channel: FixtureChannel) => void;
	onMove: (row: SlotRow, direction: -1 | 1) => void;
	onRemove: (row: SlotRow) => void;
	onDragStart: (key: string) => void;
	onDropOn: (key: string) => void;
	onTouchMove: (sourceKey: string, targetKey: string) => void;
};

function SlotTableRow({ mode, row, index, count, attributeRegistry, ...on }: RowProps) {
	const { channel } = row;
	const coarse = row.level === 0;
	const name = channelLabel(channel, attributeRegistry);
	const key = slotRowKey(row);
	const headName =
		mode.heads.find((candidate) => candidate.id === channel.head_id)?.name ??
		"Missing head";
	const rowName = coarse
		? channel.attribute
		: `${channel.attribute} ${CHANNEL_LEVELS[row.level].toLowerCase()}`;
	return (
		<tr
			className={`fixture-channel-row${coarse ? "" : " is-byte"}`}
			data-slot-row-key={key}
			draggable
			onDragStart={() => on.onDragStart(key)}
			onDragOver={(event) => event.preventDefault()}
			onDrop={(event) => {
				event.preventDefault();
				on.onDropOn(key);
			}}
		>
			<td className="channel-primary-slot">
				<span className="fixture-slot-cell">
					<TouchSlotDragHandle rowKey={key} onMove={on.onTouchMove} />
					<span className="fixture-slot-number">{row.slot}</span>
				</span>
			</td>
			<td>
				{coarse ? (
					<ValueCell
						label={`Head for slot ${row.slot}: ${headName}`}
						onPress={() => on.onEdit({ kind: "head", channelId: channel.id })}
					>
						{headName}
					</ValueCell>
				) : (
					<span className="fixture-slot-inherited">{headName}</span>
				)}
			</td>
			<td>
				{coarse ? (
					<ValueCell
						label={`Attribute for slot ${row.slot}: ${name}`}
						onPress={() => on.onEdit({ kind: "attribute", channelId: channel.id })}
					>
						{name}
					</ValueCell>
				) : (
					<span className="fixture-slot-inherited">↳ {name}</span>
				)}
			</td>
			<td>
				<ValueCell
					label={`Level for slot ${row.slot}: ${CHANNEL_LEVELS[row.level]}`}
					onPress={() => on.onEdit({ kind: "level", rowKey: key })}
				>
					{CHANNEL_LEVELS[row.level]}
				</ValueCell>
			</td>
			<ChannelSettingCells
				row={row}
				name={name}
				attributeRegistry={attributeRegistry}
				onChannel={on.onChannel}
				onEdit={on.onEdit}
				onEditMapping={on.onEditMapping}
			/>
			<td>
				<SlotOrderActions
					name={rowName}
					first={index === 0}
					last={index === count - 1}
					onMove={(direction) => on.onMove(row, direction)}
					onRemove={() => on.onRemove(row)}
				/>
			</td>
		</tr>
	);
}

function RawValueDialog({
	channel,
	field,
	name,
	onChannel,
	onClose,
}: {
	channel: FixtureChannel;
	field: RawField;
	name: string;
	onChannel: (channel: FixtureChannel) => void;
	onClose: () => void;
}) {
	const max = maxRaw(channel.resolution);
	return (
		<InputModal
			kind="number"
			label={`${name} ${field === "default_raw" ? "default" : "highlight"} (0–${max})`}
			value={String(channel[field])}
			onCommit={(value) => {
				const raw = Math.round(Number(value));
				if (Number.isFinite(raw))
					onChannel({ ...channel, [field]: Math.min(max, Math.max(0, raw)) });
				onClose();
			}}
			onCancel={onClose}
		/>
	);
}

/** The window a row's setting opens: attribute, head, level, keypad, or masters. */
function SlotEditDialog({
	edit,
	mode,
	rows,
	attributeRegistry,
	onChannel,
	onLevel,
	onClose,
}: {
	edit: SlotEdit;
	mode: FixtureMode;
	rows: SlotRow[];
	attributeRegistry: AttributeDescriptor[];
	onChannel: (channel: FixtureChannel) => void;
	onLevel: (row: SlotRow, level: ChannelLevel) => void;
	onClose: () => void;
}) {
	if (edit.kind === "level") {
		const row = rows.find((candidate) => slotRowKey(candidate) === edit.rowKey);
		if (!row) return null;
		return (
			<OptionPickerModal
				title={`Level · slot ${row.slot}`}
				value={String(row.level)}
				options={CHANNEL_LEVELS.map((label, value) => ({ value: String(value), label }))}
				onSelect={(value) => onLevel(row, Number(value) as ChannelLevel)}
				onClose={onClose}
			/>
		);
	}
	const channel = mode.channels.find((candidate) => candidate.id === edit.channelId);
	if (!channel) return null;
	const name = channelLabel(channel, attributeRegistry);
	const slot = rows.find((row) => row.channel.id === channel.id && row.level === 0)?.slot;
	if (edit.kind === "head")
		return (
			<OptionPickerModal
				title={`Head · slot ${slot ?? ""}`}
				value={channel.head_id}
				options={mode.heads.map((head) => ({ value: head.id, label: head.name }))}
				onSelect={(head_id) => onChannel({ ...channel, head_id })}
				onClose={onClose}
			/>
		);
	if (edit.kind === "attribute")
		return (
			<AttributePickerModal
				title={`Attribute · slot ${slot ?? ""}`}
				value={channel.behavior === "static" ? STATIC_ATTRIBUTE : channel.attribute}
				registry={attributeRegistry}
				onSelect={(attribute) => {
					onChannel(
						attribute === STATIC_ATTRIBUTE
							? { ...channel, behavior: "static" }
							: {
									...applyCanonicalChannelAttribute(channel, attribute, attributeRegistry),
									behavior: "controlled",
								},
					);
					onClose();
				}}
				onClose={onClose}
			/>
		);
	if (edit.kind === "masters")
		return (
			<MastersModal channel={channel} label={name} onChange={onChannel} onClose={onClose} />
		);
	return (
		<RawValueDialog
			channel={channel}
			field={edit.field}
			name={name}
			onChannel={onChannel}
			onClose={onClose}
		/>
	);
}

/** Removing a slot closes up the slots after it, so it is asked about first. */
function RemoveSlotDialog({
	row,
	attributeRegistry,
	onRemove,
	onClose,
}: {
	row: SlotRow | null;
	attributeRegistry: AttributeDescriptor[];
	onRemove: (row: SlotRow) => void;
	onClose: () => void;
}) {
	if (!row) return null;
	const name = channelLabel(row.channel, attributeRegistry);
	return (
		<ConfirmDialog
			title={`Remove slot ${row.slot}?`}
			description={
				row.level === 0
					? `Removes ${name} with every byte of it. The slots after it move up.`
					: `Removes this ${CHANNEL_LEVELS[row.level]} byte; ${name} keeps its other bytes. The slots after it move up.`
			}
			primary="Remove slot"
			danger
			onPrimary={() => onRemove(row)}
			secondary="Keep slot"
			onSecondary={onClose}
		/>
	);
}

/**
 * One split's DMX slots, in slot order, each with its level.
 *
 * Every cell shows its value as text and opens a window to change it, so a row stays one line.
 * Only a Coarse row carries the channel's settings. A Fine, Ultra or Extreme row is a further byte
 * of the coarse channel with the same attribute on the same head, so it shows what it refines and
 * leaves the rest to that row.
 */
export function SlotTable({
	mode,
	split,
	attributeRegistry,
	onChange,
	onEditMapping,
}: {
	mode: FixtureMode;
	split: number;
	attributeRegistry: AttributeDescriptor[];
	onChange: (mode: FixtureMode) => void;
	onEditMapping: (channel: FixtureChannel) => void;
}) {
	const [dragKey, setDragKey] = useState<string | null>(null);
	const [removingKey, setRemovingKey] = useState<string | null>(null);
	const [levelError, setLevelError] = useState<string | null>(null);
	const [edit, setEdit] = useState<SlotEdit | null>(null);
	const rows = slotRows(mode, split);
	const onChannel = (channel: FixtureChannel) => onChange(replaceChannel(mode, channel));
	const moveByKey = (sourceKey: string, targetKey: string) => {
		const source = rows.find((row) => slotRowKey(row) === sourceKey);
		const target = rows.find((row) => slotRowKey(row) === targetKey);
		const next = source && target && moveSlotRowTo(mode, split, source, target);
		if (next) onChange(next);
	};
	const onLevel = (row: SlotRow, level: ChannelLevel) => {
		const result = setRowLevel(mode, split, row, level, (attribute) =>
			channelLabel({ ...row.channel, attribute, behavior: "controlled" }, attributeRegistry),
		);
		setLevelError(result.error ?? null);
		if (result.mode) onChange(result.mode);
	};
	return (
		<div className="fixture-channel-split">
			{levelError && (
				<p className="fixture-inline-errors" role="alert">
					{levelError}
				</p>
			)}
			<div className="fixture-channel-table-wrap">
				<table className="fixture-channel-table fixture-slot-table">
					<thead>
						<tr>
							{COLUMNS.map((column) => (
								<th key={column}>{column}</th>
							))}
						</tr>
					</thead>
					<tbody>
						{rows.map((row, index) => (
							<SlotTableRow
								key={slotRowKey(row)}
								mode={mode}
								row={row}
								index={index}
								count={rows.length}
								attributeRegistry={attributeRegistry}
								onChannel={onChannel}
								onEdit={setEdit}
								onEditMapping={onEditMapping}
								onMove={(target, direction) => {
									const next = moveSlotRow(mode, split, target, direction);
									if (next) onChange(next);
								}}
								onRemove={(target) => setRemovingKey(slotRowKey(target))}
								onDragStart={setDragKey}
								onDropOn={(key) => {
									if (dragKey) moveByKey(dragKey, key);
									setDragKey(null);
								}}
								onTouchMove={moveByKey}
							/>
						))}
					</tbody>
				</table>
			</div>
			{!rows.length && (
				<p className="empty-editor-message">
					No logical channels are assigned to split {split}. Add one from the title bar.
				</p>
			)}
			<RemoveSlotDialog
				row={rows.find((row) => slotRowKey(row) === removingKey) ?? null}
				attributeRegistry={attributeRegistry}
				onRemove={(row) => {
					onChange(removeSlotRow(mode, split, row));
					setRemovingKey(null);
				}}
				onClose={() => setRemovingKey(null)}
			/>
			{edit && (
				<SlotEditDialog
					edit={edit}
					mode={mode}
					rows={rows}
					attributeRegistry={attributeRegistry}
					onChannel={onChannel}
					onLevel={onLevel}
					onClose={() => setEdit(null)}
				/>
			)}
		</div>
	);
}
