import type { FixtureChannel, FixtureMode } from "../../wire";

const COLOR_EMITTERS = new Set([
	"color.red",
	"color.green",
	"color.blue",
	"color.white",
	"color.amber",
	"color.uv",
	"color.lime",
	"color.indigo",
	"color.mint",
	"color.cold_white",
	"color.warm_white",
	"color.brightness",
]);

/**
 * A colour emitter adds light of its own colour, so scaling it scales the light. Subtractive flags,
 * wheels and colour-temperature controls are not emitters. Mirrors the server's
 * `AttributeKey::is_color_emitter`.
 */
export function isColorEmitterAttribute(attribute: string | undefined) {
	return attribute !== undefined && COLOR_EMITTERS.has(attribute);
}

function isIntensityAttribute(attribute: string | undefined) {
	return attribute === "intensity" || Boolean(attribute?.endsWith(".intensity"));
}

function carriesIntensity(channel: FixtureChannel) {
	return (
		isIntensityAttribute(channel.attribute) ||
		isIntensityAttribute(channel.fixture_attribute)
	);
}

/**
 * A light-emitting head without an Intensity channel has a virtual dimmer: an Intensity the desk
 * programs and the masters scale, reaching the light through the channels following it. Mirrors
 * the server's `FixtureMode::head_has_virtual_dimmer`.
 */
export function headHasVirtualDimmer(mode: FixtureMode, headId: string) {
	const channels = mode.channels.filter((channel) => channel.head_id === headId);
	return (
		!channels.some(carriesIntensity) &&
		channels.some(
			(channel) =>
				channel.reacts_to_virtual_intensity ||
				// A hand-built channel may name only its canonical attribute.
				isColorEmitterAttribute(channel.fixture_attribute ?? channel.attribute),
		)
	);
}

/**
 * The virtual-dimmer reaction a channel starts with once its attribute is chosen: a colour emitter
 * on a head without an Intensity channel follows the virtual dimmer, one on a dimmed head ignores
 * it (the dimmer already dims it). Any other channel keeps the operator's choice.
 */
export function withDefaultVirtualDimmerReaction(
	mode: FixtureMode,
	channel: FixtureChannel,
	attribute: string,
): FixtureChannel {
	if (!isColorEmitterAttribute(attribute)) return channel;
	const dimmed = mode.channels.some(
		(other) =>
			other.id !== channel.id &&
			other.head_id === channel.head_id &&
			carriesIntensity(other),
	);
	return {
		...channel,
		reacts_to_virtual_intensity: !dimmed,
		virtual_intensity_inverted: false,
	};
}
