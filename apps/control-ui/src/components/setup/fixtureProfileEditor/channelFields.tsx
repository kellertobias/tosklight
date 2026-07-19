import type {
	AttributeDescriptor,
	ChannelResolution,
	FixtureChannel,
	FixtureMode,
} from "../../../api/types";
import {
	CheckboxField,
	FormLayout,
	NumberField,
	SelectField,
	TextField,
} from "../../common";
import { maxRaw, resolutionBytes } from "../fixtureProfileModel";
import {
	applyCanonicalChannelAttribute,
	attributeOptions,
} from "./channelModel";

const RESOLUTIONS: ChannelResolution[] = ["u8", "u16", "u24", "u32"];

function optionalNumber(value: string) {
	return value === "" ? null : Number(value);
}

export function ChannelCoreFields({
	mode,
	channel,
	attributeRegistry,
	onChange,
	onResolution,
}: {
	mode: FixtureMode;
	channel: FixtureChannel;
	attributeRegistry: AttributeDescriptor[];
	onChange: (channel: FixtureChannel) => void;
	onResolution: (resolution: ChannelResolution) => void;
}) {
	const components = resolutionBytes(channel.resolution) - 1;
	const role = channel.behavior === "static" ? "__static" : channel.attribute;
	const setRole = (value: string) =>
		onChange(
			value === "__static"
				? { ...channel, behavior: "static" }
				: {
						...applyCanonicalChannelAttribute(
							channel,
							value,
							attributeRegistry,
						),
						behavior: "controlled",
					},
		);
	const setSecondary = (index: number, value: number) => {
		const secondary_slots = [...channel.secondary_slots];
		secondary_slots[index] = value;
		onChange({ ...channel, secondary_slots });
	};
	return (
		<FormLayout columns={3} minColumnWidth={210}>
			<SelectField
				label="Channel role"
				value={role}
				options={[
					{ value: "__static", label: "Static output" },
					...attributeOptions(attributeRegistry, channel.attribute),
				]}
				onChange={setRole}
			/>
			<SelectField
				label="Address split"
				value={String(channel.split)}
				options={mode.splits.map((split) => ({
					value: String(split.number),
					label: `Split ${split.number}`,
				}))}
				onChange={(value) => onChange({ ...channel, split: Number(value) })}
			/>
			<SelectField
				label="Logical head"
				value={channel.head_id}
				options={mode.heads.map((head) => ({
					value: head.id,
					label: head.name,
				}))}
				onChange={(head_id) => onChange({ ...channel, head_id })}
			/>
			<SelectField
				label="Resolution"
				value={channel.resolution}
				options={RESOLUTIONS.map((value) => ({
					value,
					label: `${value.slice(1)} bit`,
				}))}
				onChange={onResolution}
			/>
			{[0, 1, 2].map((index) =>
				components > index ? (
					<NumberField
						key={index}
						label={`${["Fine", "Third byte", "Fourth byte"][index]} slot`}
						min={1}
						max={512}
						value={channel.secondary_slots[index] ?? ""}
						onChange={(event) =>
							setSecondary(index, Number(event.target.value))
						}
					/>
				) : null,
			)}
			<NumberField
				label="Default raw"
				min={0}
				max={maxRaw(channel.resolution)}
				value={channel.default_raw}
				onChange={(event) =>
					onChange({ ...channel, default_raw: Number(event.target.value) })
				}
			/>
			<NumberField
				label="Highlight raw"
				min={0}
				max={maxRaw(channel.resolution)}
				value={channel.highlight_raw}
				onChange={(event) =>
					onChange({ ...channel, highlight_raw: Number(event.target.value) })
				}
			/>
			<NumberField
				label="Physical minimum"
				allowDecimal
				value={channel.physical_min ?? ""}
				onChange={(event) =>
					onChange({
						...channel,
						physical_min: optionalNumber(event.target.value),
					})
				}
			/>
			<NumberField
				label="Physical maximum"
				allowDecimal
				value={channel.physical_max ?? ""}
				onChange={(event) =>
					onChange({
						...channel,
						physical_max: optionalNumber(event.target.value),
					})
				}
			/>
			<TextField
				label="Physical unit"
				value={channel.unit ?? ""}
				onChange={(event) =>
					onChange({ ...channel, unit: event.target.value || null })
				}
			/>
		</FormLayout>
	);
}

const CHANNEL_FLAGS = [
	["invert", "Invert"],
	["snap", "Snap (never fades)"],
	["reacts_to_virtual_intensity", "Reacts to virtual intensity"],
	["reacts_to_sequence_master", "Reacts to sequence master"],
	["reacts_to_group_master", "Reacts to group master"],
	["reacts_to_grand_master", "Reacts to grand master"],
] as const;

export function ChannelFields({
	channel,
	onChange,
}: {
	channel: FixtureChannel;
	onChange: (channel: FixtureChannel) => void;
}) {
	return (
		<div className="fixture-channel-flags">
			{CHANNEL_FLAGS.map(([key, label]) => (
				<CheckboxField
					key={key}
					label={label}
					checked={channel[key]}
					onChange={(event) =>
						onChange({ ...channel, [key]: event.target.checked })
					}
				/>
			))}
		</div>
	);
}
