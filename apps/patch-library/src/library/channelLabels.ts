import type { AttributeDescriptor, FixtureChannel } from "../wire";

/** What the operator calls a channel: its attribute's name, or Static. */
export function channelLabel(
	channel: FixtureChannel,
	registry: readonly AttributeDescriptor[],
) {
	if (channel.behavior === "static") return "Static";
	return (
		registry.find((descriptor) => descriptor.id === channel.attribute)?.label ??
		(channel.attribute || "Unassigned")
	);
}

/**
 * The unit a channel's physical range is in.
 *
 * It comes from the attribute, not from the operator: Pan is in degrees whatever the fixture, so
 * the range is typed as numbers and the unit follows the attribute chosen. Only an attribute the
 * registry does not know keeps the unit the profile already carried.
 */
export function channelUnit(
	channel: FixtureChannel,
	registry: readonly AttributeDescriptor[],
) {
	const descriptor = registry.find(
		(candidate) => candidate.id === channel.attribute,
	);
	if (!descriptor) return channel.unit;
	return (
		descriptor.physical_unit ??
		descriptor.default_unit ??
		descriptor.display_unit ??
		null
	);
}

function number(value: number | null) {
	return value == null ? "?" : String(value);
}

/** A channel's range and function count, short enough for a table cell. */
export function mappingSummary(channel: FixtureChannel, unit: string | null) {
	const range =
		channel.physical_min == null && channel.physical_max == null
			? "No range"
			: `${number(channel.physical_min)}–${number(channel.physical_max)}${unit ? ` ${unit}` : ""}`;
	const count = channel.functions.length;
	return `${range} · ${count} ${count === 1 ? "function" : "functions"}`;
}
