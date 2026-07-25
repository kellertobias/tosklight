import type {
	AttributeDescriptor,
	ChannelFunctionBehavior,
	FixtureChannel,
} from "../../../api/types";
import { Button, NumberField, SelectField, TextField } from "../../common";
import { maxRaw } from "../fixtureProfileModel";
import { attributeOptions, replaceFunctionBehavior } from "./channelModel";

type ChannelFunction = FixtureChannel["functions"][number];

function FunctionBaseFields({
	fn,
	channel,
	attributeRegistry,
	onChange,
}: {
	fn: ChannelFunction;
	channel: FixtureChannel;
	attributeRegistry: AttributeDescriptor[];
	onChange: (fn: ChannelFunction) => void;
}) {
	return (
		<>
			<TextField
				label="Function name"
				value={fn.name}
				onChange={(event) => onChange({ ...fn, name: event.target.value })}
			/>
			<SelectField
				label="Function attribute"
				value={fn.attribute}
				options={attributeOptions(attributeRegistry, fn.attribute)}
				onChange={(attribute) => onChange({ ...fn, attribute })}
			/>
			<NumberField
				label="DMX from"
				min={0}
				max={maxRaw(channel.resolution)}
				value={fn.dmx_from}
				onChange={(event) =>
					onChange({ ...fn, dmx_from: Number(event.target.value) })
				}
			/>
			<NumberField
				label="DMX to"
				min={0}
				max={maxRaw(channel.resolution)}
				value={fn.dmx_to}
				onChange={(event) =>
					onChange({ ...fn, dmx_to: Number(event.target.value) })
				}
			/>
			<NumberField
				label="Priority"
				value={fn.priority}
				onChange={(event) =>
					onChange({ ...fn, priority: Number(event.target.value) })
				}
			/>
			<SelectField
				label="Function behavior"
				value={fn.behavior.type}
				options={[
					{ value: "continuous", label: "Continuous mapping" },
					{ value: "fixed", label: "Named fixed value" },
					{ value: "indexed", label: "Indexed color or gobo" },
					{ value: "control", label: "Control action" },
				]}
				onChange={(type) =>
					onChange(replaceFunctionBehavior(fn, type, channel))
				}
			/>
		</>
	);
}

function FunctionBehaviorEditor({
	behavior,
	modeChannel,
	actionIds,
	onChange,
}: {
	behavior: ChannelFunctionBehavior;
	modeChannel: FixtureChannel;
	actionIds: Array<{ id: string; name: string }>;
	onChange: (behavior: ChannelFunctionBehavior) => void;
}) {
	if (behavior.type === "continuous")
		return (
			<>
				<NumberField
					label="Function physical minimum"
					allowDecimal
					value={behavior.physical_min}
					onChange={(event) =>
						onChange({ ...behavior, physical_min: Number(event.target.value) })
					}
				/>
				<NumberField
					label="Function physical maximum"
					allowDecimal
					value={behavior.physical_max}
					onChange={(event) =>
						onChange({ ...behavior, physical_max: Number(event.target.value) })
					}
				/>
				<TextField
					label="Function unit"
					value={behavior.unit ?? ""}
					onChange={(event) =>
						onChange({ ...behavior, unit: event.target.value || null })
					}
				/>
			</>
		);
	if (behavior.type === "control")
		return (
			<SelectField
				label="Control action"
				value={behavior.action_id}
				options={[
					{ value: "", label: "Choose action" },
					...actionIds.map((action) => ({
						value: action.id,
						label: action.name,
					})),
				]}
				onChange={(action_id) => onChange({ ...behavior, action_id })}
			/>
		);
	return (
		<>
			<TextField
				label="Portable semantic ID"
				value={behavior.semantic_id}
				onChange={(event) =>
					onChange({ ...behavior, semantic_id: event.target.value })
				}
			/>
			<TextField
				label="Fixture label"
				value={behavior.label}
				onChange={(event) =>
					onChange({ ...behavior, label: event.target.value })
				}
			/>
			<NumberField
				label="Exact raw value"
				min={0}
				max={maxRaw(modeChannel.resolution)}
				value={behavior.raw_value}
				onChange={(event) =>
					onChange({ ...behavior, raw_value: Number(event.target.value) })
				}
			/>
		</>
	);
}

function FunctionActions({
	fn,
	index,
	count,
	onMove,
	onRemove,
}: {
	fn: ChannelFunction;
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
			<Button onClick={onRemove}>Remove function</Button>
		</div>
	);
}

export function ChannelFunctionCard({
	fn,
	index,
	channel,
	attributeRegistry,
	actionIds,
	onChange,
	onMove,
	onRemove,
}: {
	fn: ChannelFunction;
	index: number;
	channel: FixtureChannel;
	attributeRegistry: AttributeDescriptor[];
	actionIds: Array<{ id: string; name: string }>;
	onChange: (fn: ChannelFunction) => void;
	onMove: (offset: -1 | 1) => void;
	onRemove: () => void;
}) {
	return (
		<article>
			<FunctionBaseFields
				fn={fn}
				channel={channel}
				attributeRegistry={attributeRegistry}
				onChange={onChange}
			/>
			<FunctionBehaviorEditor
				behavior={fn.behavior}
				modeChannel={channel}
				actionIds={actionIds}
				onChange={(behavior) => onChange({ ...fn, behavior })}
			/>
			<FunctionActions
				fn={fn}
				index={index}
				count={channel.functions.length}
				onMove={onMove}
				onRemove={onRemove}
			/>
		</article>
	);
}
