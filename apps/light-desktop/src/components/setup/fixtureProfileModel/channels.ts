import type {
	ChannelFunction,
	ChannelResolution,
	FixtureChannel,
	FixtureMode,
} from "../../../api/types";
import { semanticHighlightRaw } from "./color";
import { maxRaw, resolutionBytes } from "./rawValues";
import { uuid } from "./utilities";

export function channelSplit(_mode: FixtureMode, channel: FixtureChannel) {
	return channel.split;
}

function reserveSecondarySlots(mode: FixtureMode, errors: string[]) {
	const footprints = new Map(
		mode.splits.map((split) => [split.number, split.footprint]),
	);
	const reserved = new Map<number, Set<number>>();
	for (const channel of mode.channels) {
		const split = channelSplit(mode, channel);
		const footprint = footprints.get(split) ?? 0;
		const expected = resolutionBytes(channel.resolution) - 1;
		if (channel.secondary_slots.length !== expected) {
			errors.push(
				`${channel.attribute || "Channel"}: ${channel.resolution.slice(1)}-bit resolution needs ${expected} component slots`,
			);
		}
		const used = reserved.get(split) ?? new Set<number>();
		for (const slot of channel.secondary_slots) {
			if (!Number.isInteger(slot) || slot < 1 || slot > footprint) {
				errors.push(
					`${channel.attribute || "Channel"}: component slot ${slot} is outside split ${split}`,
				);
			}
			if (used.has(slot)) {
				errors.push(`Split ${split}: DMX component slot ${slot} is duplicated`);
			}
			used.add(slot);
		}
		reserved.set(split, used);
	}
	return { footprints, reserved };
}

export function derivePrimarySlots(
	mode: FixtureMode,
): { slots: Map<string, number>; errors: string[] } {
	const errors: string[] = [];
	const { footprints, reserved } = reserveSecondarySlots(mode, errors);
	const next = new Map<number, number>();
	const primaryUsed = new Map<number, Set<number>>();
	const slots = new Map<string, number>();
	for (const channel of mode.channels) {
		const split = channelSplit(mode, channel);
		const footprint = footprints.get(split) ?? 0;
		const occupied = reserved.get(split) ?? new Set<number>();
		const used = primaryUsed.get(split) ?? new Set<number>();
		let candidate = next.get(split) ?? 1;
		while (occupied.has(candidate) || used.has(candidate)) candidate += 1;
		if (!split || candidate > footprint) {
			errors.push(
				`${channel.attribute || "Channel"}: split ${split || "?"} exceeds its ${footprint}-slot footprint`,
			);
		}
		slots.set(channel.id, candidate);
		used.add(candidate);
		primaryUsed.set(split, used);
		next.set(split, candidate + 1);
	}
	return { slots, errors: [...new Set(errors)] };
}

export function blankChannel(
	mode: FixtureMode,
	split = mode.splits[0]?.number ?? 1,
): FixtureChannel {
	const head = mode.heads[0];
	const resolution: ChannelResolution = "u8";
	const defaultRaw = 0;
	return {
		id: uuid(),
		head_id: head?.id ?? "",
		split,
		attribute: "intensity",
		resolution,
		secondary_slots: [],
		default_raw: defaultRaw,
		highlight_raw: semanticHighlightRaw(
			"intensity",
			resolution,
			defaultRaw,
		),
		physical_min: 0,
		physical_max: 100,
		unit: "percent",
		invert: false,
		snap: false,
		reacts_to_virtual_intensity: false,
		reacts_to_sequence_master: true,
		reacts_to_group_master: true,
		reacts_to_grand_master: true,
		behavior: "controlled",
		functions: [],
	};
}

export function blankFunction(
	channel: FixtureChannel,
	type: ChannelFunction["behavior"]["type"] = "continuous",
): ChannelFunction {
	const range = maxRaw(channel.resolution);
	const behavior: ChannelFunction["behavior"] =
		type === "continuous"
			? {
					type,
					physical_min: channel.physical_min ?? 0,
					physical_max: channel.physical_max ?? 1,
					unit: channel.unit,
				}
			: type === "control"
				? { type, action_id: "" }
				: { type, semantic_id: "", label: "", raw_value: 0 };
	return {
		id: uuid(),
		name: type === "continuous" ? channel.attribute : "Function",
		dmx_from: 0,
		dmx_to: range,
		attribute: channel.attribute,
		priority: type === "continuous" ? 0 : type === "control" ? 200 : 100,
		behavior,
	};
}
