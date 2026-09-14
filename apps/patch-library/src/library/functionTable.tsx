import { Button, InputModal } from "@tosklight/ui";
import { Fragment, useState } from "react";
import type {
	AttributeDescriptor,
	ChannelFunctionBehavior,
	FixtureChannel,
} from "../wire";
import { blankFunction, maxRaw, reorder } from "../sheet/fixtureProfileModel";
import { AttributePickerModal } from "./attributePicker";
import {
	AngularMotionEditor,
	FunctionBehaviorEditor,
} from "./channelFunctionCard";
import { replaceFunctionBehavior } from "./channelModel";
import { OptionPickerModal } from "./optionPicker";

type ChannelFunction = FixtureChannel["functions"][number];
type ActionIds = Array<{ id: string; name: string }>;
type FunctionField = "name" | "attribute" | "dmx_from" | "dmx_to" | "behavior" | "priority";
type FunctionEdit = { functionId: string; field: FunctionField };

const BEHAVIORS: { value: ChannelFunctionBehavior["type"]; label: string }[] = [
	{ value: "continuous", label: "Continuous mapping" },
	{ value: "fixed", label: "Named fixed value" },
	{ value: "indexed", label: "Indexed color or gobo" },
	{ value: "control", label: "Control action" },
];

const COLUMNS = [
	"Name",
	"Attribute",
	"DMX from",
	"DMX to",
	"Behavior",
	"Priority",
	"Details",
	"Order",
];

/** The window one of a function's cells opens. */
function FunctionEditDialog({
	edit,
	channel,
	attributeRegistry,
	onFunction,
	onClose,
}: {
	edit: FunctionEdit;
	channel: FixtureChannel;
	attributeRegistry: AttributeDescriptor[];
	onFunction: (fn: ChannelFunction) => void;
	onClose: () => void;
}) {
	const fn = channel.functions.find((candidate) => candidate.id === edit.functionId);
	if (!fn) return null;
	if (edit.field === "attribute")
		return (
			<AttributePickerModal
				title="Function attribute"
				value={fn.attribute}
				registry={attributeRegistry}
				onSelect={(attribute) => {
					onFunction({ ...fn, attribute });
					onClose();
				}}
				onClose={onClose}
			/>
		);
	if (edit.field === "behavior")
		return (
			<OptionPickerModal
				title="Function behavior"
				value={fn.behavior.type}
				options={BEHAVIORS}
				onSelect={(type) =>
					onFunction(
						replaceFunctionBehavior(fn, type as ChannelFunctionBehavior["type"], channel),
					)
				}
				onClose={onClose}
			/>
		);
	if (edit.field === "name")
		return (
			<InputModal
				kind="text"
				label="Function name"
				value={fn.name}
				onCommit={(name) => {
					onFunction({ ...fn, name });
					onClose();
				}}
				onCancel={onClose}
			/>
		);
	const field = edit.field;
	const max = field === "priority" ? null : maxRaw(channel.resolution);
	return (
		<InputModal
			kind="number"
			label={
				field === "priority"
					? "Priority"
					: `${field === "dmx_from" ? "DMX from" : "DMX to"} (0–${max})`
			}
			value={String(fn[field])}
			onCommit={(value) => {
				const number = Math.round(Number(value));
				if (Number.isFinite(number))
					onFunction({
						...fn,
						[field]: max == null ? number : Math.min(max, Math.max(0, number)),
					});
				onClose();
			}}
			onCancel={onClose}
		/>
	);
}

function FunctionActions({
	fn,
	name,
	index,
	count,
	onMove,
	onRemove,
}: {
	fn: ChannelFunction;
	name: string;
	index: number;
	count: number;
	onMove: (offset: -1 | 1) => void;
	onRemove: () => void;
}) {
	return (
		<div className="reorder-actions">
			<Button
				iconOnly
				aria-label={`Move function ${fn.name} up`}
				disabled={index === 0}
				onClick={() => onMove(-1)}
			>
				▲
			</Button>
			<Button
				iconOnly
				aria-label={`Move function ${fn.name} down`}
				disabled={index === count - 1}
				onClick={() => onMove(1)}
			>
				▼
			</Button>
			<Button iconOnly aria-label={`Remove ${name}`} onClick={onRemove}>
				×
			</Button>
		</div>
	);
}

/** One function, and — while open — what only its behaviour carries, on the row beneath it. */
function FunctionRow({
	fn,
	index,
	channel,
	attributeRegistry,
	actionIds,
	open,
	onToggle,
	onEdit,
	onChange,
}: {
	fn: ChannelFunction;
	index: number;
	channel: FixtureChannel;
	attributeRegistry: AttributeDescriptor[];
	actionIds: ActionIds;
	open: boolean;
	onToggle: () => void;
	onEdit: (field: FunctionField) => void;
	onChange: (channel: FixtureChannel) => void;
}) {
	const name = fn.name || `function ${index + 1}`;
	const setFunction = (next: ChannelFunction) =>
		onChange({
			...channel,
			functions: channel.functions.map((candidate) =>
				candidate.id === next.id ? next : candidate,
			),
		});
	const attribute =
		attributeRegistry.find((descriptor) => descriptor.id === fn.attribute)?.label ??
		fn.attribute;
	const cell = (field: FunctionField, column: string, value: React.ReactNode) => (
		<td>
			<Button
				className="fixture-cell-value"
				aria-label={`${column} of ${name}`}
				onClick={() => onEdit(field)}
			>
				{value === "" ? <span className="fixture-slot-inherited">Unnamed</span> : value}
			</Button>
		</td>
	);
	return (
		<Fragment>
			<tr className="fixture-function-row">
				{cell("name", "Name", fn.name)}
				{cell("attribute", "Attribute", attribute)}
				{cell("dmx_from", "DMX from", fn.dmx_from)}
				{cell("dmx_to", "DMX to", fn.dmx_to)}
				{cell(
					"behavior",
					"Behavior",
					BEHAVIORS.find((behavior) => behavior.value === fn.behavior.type)?.label,
				)}
				{cell("priority", "Priority", fn.priority)}
				<td>
					<Button aria-expanded={open} aria-label={`Details for ${name}`} onClick={onToggle}>
						{open ? "Hide" : "Details"}
					</Button>
				</td>
				<td>
					<FunctionActions
						fn={fn}
						name={name}
						index={index}
						count={channel.functions.length}
						onMove={(offset) =>
							onChange({
								...channel,
								functions: reorder(channel.functions, index, index + offset),
							})
						}
						onRemove={() =>
							onChange({
								...channel,
								functions: channel.functions.filter((candidate) => candidate.id !== fn.id),
							})
						}
					/>
				</td>
			</tr>
			{open && (
				<tr className="fixture-function-details">
					<td colSpan={COLUMNS.length}>
						<div className="fixture-function-details-grid">
							<FunctionBehaviorEditor
								behavior={fn.behavior}
								modeChannel={channel}
								actionIds={actionIds}
								onChange={(behavior) => setFunction({ ...fn, behavior })}
							/>
							<AngularMotionEditor functionValue={fn} onChange={setFunction} />
						</div>
					</td>
				</tr>
			)}
		</Fragment>
	);
}

/**
 * A channel's DMX ranges, one row each.
 *
 * Each cell shows its value and opens a window to change it. What only some behaviours carry (a
 * fixed value's label, a continuous range's physical scale, angular motion) opens under the row.
 */
export function FunctionTable({
	channel,
	attributeRegistry,
	actionIds,
	onChange,
}: {
	channel: FixtureChannel;
	attributeRegistry: AttributeDescriptor[];
	actionIds: ActionIds;
	onChange: (channel: FixtureChannel) => void;
}) {
	const [openId, setOpenId] = useState<string | null>(null);
	const [edit, setEdit] = useState<FunctionEdit | null>(null);
	const add = () => {
		const fn = blankFunction(channel);
		onChange({ ...channel, functions: [...channel.functions, fn] });
		setOpenId(fn.id);
	};
	const setFunction = (next: ChannelFunction) =>
		onChange({
			...channel,
			functions: channel.functions.map((candidate) =>
				candidate.id === next.id ? next : candidate,
			),
		});
	return (
		<div className="fixture-function-table">
			<header>
				<h3>Functions</h3>
				<small>
					DMX ranges in {channel.resolution.slice(1)}-bit raw values, 0–
					{maxRaw(channel.resolution)}
				</small>
				<Button onClick={add}>Add function</Button>
			</header>
			{!channel.functions.length ? (
				<p className="empty-editor-message">
					No functions are configured for this channel.
				</p>
			) : (
				<div className="fixture-channel-table-wrap">
					<table className="fixture-channel-table fixture-functions-table">
						<thead>
							<tr>
								{COLUMNS.map((column) => (
									<th key={column}>{column}</th>
								))}
							</tr>
						</thead>
						<tbody>
							{channel.functions.map((fn, index) => (
								<FunctionRow
									key={fn.id}
									fn={fn}
									index={index}
									channel={channel}
									attributeRegistry={attributeRegistry}
									actionIds={actionIds}
									open={openId === fn.id}
									onToggle={() => setOpenId(openId === fn.id ? null : fn.id)}
									onEdit={(field) => setEdit({ functionId: fn.id, field })}
									onChange={onChange}
								/>
							))}
						</tbody>
					</table>
				</div>
			)}
			{edit && (
				<FunctionEditDialog
					edit={edit}
					channel={channel}
					attributeRegistry={attributeRegistry}
					onFunction={setFunction}
					onClose={() => setEdit(null)}
				/>
			)}
		</div>
	);
}
