import type {
	ChannelResolution,
	FixtureChannel,
	FixtureMode,
	HeadColorSystem,
} from "../../../api/types";
import {
	blankChannel,
	channelSplit,
	derivePrimarySlots,
	maxRaw,
	reorder,
	resolutionBytes,
} from "../fixtureProfileModel";
import { removeColorChannel } from "./colorEditor";

export function replaceChannel(
	mode: FixtureMode,
	channel: FixtureChannel,
): FixtureMode {
	return {
		...mode,
		channels: mode.channels.map((candidate) =>
			candidate.id === channel.id ? channel : candidate,
		),
	};
}

export function addChannel(mode: FixtureMode, split: number): FixtureMode {
	const channel = blankChannel(mode, split);
	return {
		...mode,
		splits: mode.splits.map((candidate) =>
			candidate.number === split
				? {
						...candidate,
						footprint: Math.min(
							512,
							Math.max(
								candidate.footprint,
								mode.channels.filter(
									(item) => channelSplit(mode, item) === split,
								).length + 1,
							),
						),
					}
				: candidate,
		),
		channels: [...mode.channels, channel],
	};
}

export function changeChannelResolution(
	mode: FixtureMode,
	channel: FixtureChannel,
	resolution: ChannelResolution,
): FixtureMode {
	const split = channelSplit(mode, channel);
	const footprint =
		mode.splits.find((candidate) => candidate.number === split)?.footprint ?? 1;
	const occupied = new Set(
		mode.channels
			.filter(
				(candidate) =>
					candidate.id !== channel.id &&
					channelSplit(mode, candidate) === split,
			)
			.flatMap((candidate) => candidate.secondary_slots),
	);
	const primarySlot = derivePrimarySlots(mode).slots.get(channel.id) ?? 1;
	occupied.add(primarySlot);
	const secondary: number[] = [];
	let candidate = 1;
	while (secondary.length < resolutionBytes(resolution) - 1) {
		while (occupied.has(candidate) || secondary.includes(candidate))
			candidate += 1;
		secondary.push(candidate);
		candidate += 1;
	}
	const neededFootprint = Math.max(footprint, primarySlot, ...secondary);
	return {
		...mode,
		splits: mode.splits.map((item) =>
			item.number === split
				? { ...item, footprint: Math.min(512, neededFootprint) }
				: item,
		),
		channels: mode.channels.map((item) =>
			item.id === channel.id
				? {
						...item,
						resolution,
						secondary_slots: secondary,
						default_raw: Math.min(item.default_raw, maxRaw(resolution)),
						highlight_raw: Math.min(item.highlight_raw, maxRaw(resolution)),
					}
				: item,
		),
	};
}

export function moveChannel(
	mode: FixtureMode,
	channel: FixtureChannel,
	direction: -1 | 1,
): FixtureMode | null {
	const splitChannels = mode.channels.filter(
		(candidate) =>
			channelSplit(mode, candidate) === channelSplit(mode, channel),
	);
	const within = splitChannels.findIndex(
		(candidate) => candidate.id === channel.id,
	);
	const peer = splitChannels[within + direction];
	if (!peer) return null;
	const from = mode.channels.findIndex(
		(candidate) => candidate.id === channel.id,
	);
	const to = mode.channels.findIndex((candidate) => candidate.id === peer.id);
	return { ...mode, channels: reorder(mode.channels, from, to) };
}

export function moveChannelById(
	mode: FixtureMode,
	sourceId: string,
	targetId: string,
): FixtureMode | null {
	const source = mode.channels.find((channel) => channel.id === sourceId);
	const target = mode.channels.find((channel) => channel.id === targetId);
	if (
		!source ||
		!target ||
		source.id === target.id ||
		channelSplit(mode, source) !== channelSplit(mode, target)
	)
		return null;
	const from = mode.channels.findIndex((channel) => channel.id === source.id);
	const to = mode.channels.findIndex((channel) => channel.id === target.id);
	return { ...mode, channels: reorder(mode.channels, from, to) };
}

export function removeChannel(
	mode: FixtureMode,
	channelId: string,
): FixtureMode {
	return {
		...mode,
		channels: mode.channels.filter((candidate) => candidate.id !== channelId),
		control_actions: mode.control_actions
			.map((action) => ({
				...action,
				assignments: action.assignments.filter(
					(assignment) => assignment.channel_id !== channelId,
				),
			}))
			.filter((action) => action.assignments.length),
		color_systems: mode.color_systems
			.map((system) => removeColorChannel(system, channelId))
			.filter((system): system is HeadColorSystem => Boolean(system)),
	};
}
