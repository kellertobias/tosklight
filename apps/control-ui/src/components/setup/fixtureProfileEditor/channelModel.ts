import type {
	AttributeDescriptor,
	ChannelFunctionBehavior,
	FixtureChannel,
} from "../../../api/types";
import { semanticHighlightRaw } from "../fixtureProfileModel";

export function applyCanonicalChannelAttribute(
	channel: FixtureChannel,
	attribute: string,
	registry: AttributeDescriptor[],
): FixtureChannel {
	const descriptor = registry.find((candidate) => candidate.id === attribute);
	const choices = channel.functions.flatMap((fn) =>
		fn.behavior.type === "fixed" || fn.behavior.type === "indexed"
			? [
					{
						semantic_id: fn.behavior.semantic_id,
						label: fn.behavior.label,
						raw_value: fn.behavior.raw_value,
					},
				]
			: [],
	);
	const previousDefault = semanticHighlightRaw(
		channel.attribute,
		channel.resolution,
		channel.default_raw,
		channel.invert,
		choices,
	);
	const nextDefault = semanticHighlightRaw(
		attribute,
		channel.resolution,
		channel.default_raw,
		channel.invert,
		choices,
	);
	return {
		...channel,
		attribute,
		highlight_raw:
			channel.highlight_raw === previousDefault
				? nextDefault
				: channel.highlight_raw,
		unit: descriptor?.default_unit ?? (descriptor ? null : channel.unit),
	};
}

export function attributeOptions(
	registry: AttributeDescriptor[],
	current: string,
) {
	const options = registry.map((descriptor) => ({
		value: descriptor.id,
		label: `${descriptor.family} · ${descriptor.label}`,
	}));
	if (current && !options.some((option) => option.value === current))
		options.push({ value: current, label: current });
	return options;
}

function functionBehavior(
	type: ChannelFunctionBehavior["type"],
	channel: FixtureChannel,
): ChannelFunctionBehavior {
	if (type === "continuous")
		return {
			type,
			physical_min: channel.physical_min ?? 0,
			physical_max: channel.physical_max ?? 1,
			unit: channel.unit,
		};
	if (type === "control") return { type, action_id: "" };
	return { type, semantic_id: "", label: "", raw_value: 0 };
}

export function replaceFunctionBehavior(
	fn: FixtureChannel["functions"][number],
	type: ChannelFunctionBehavior["type"],
	channel: FixtureChannel,
): FixtureChannel["functions"][number] {
	return {
		...fn,
		priority: type === "continuous" ? 0 : type === "control" ? 200 : 100,
		behavior: functionBehavior(type, channel),
	};
}
