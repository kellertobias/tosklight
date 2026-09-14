import type {
	ChannelResolution,
	ColorSystem,
	FixtureChannel,
	FixtureMode,
} from "../wire";
import {
	blankChannel,
	derivePrimarySlots,
	maxRaw,
	resolutionBytes,
} from "../sheet/fixtureProfileModel";
import { removeChannel } from "./channelOperations";

/**
 * The channel table as the DMX chart reads it: one row per slot.
 *
 * A 16-bit Pan is one channel in the profile — one attribute, one default, one set of functions —
 * spread over two slots. A manual lists those as "Pan" and "Pan Fine", so the table does too: the
 * channel's first slot is its Coarse row and every further byte is a row of its own, at the level
 * that byte is. Turning a row into Fine is what joins it to the coarse channel above it; turning it
 * back into Coarse is what makes it a channel again. The profile keeps storing exactly what it
 * always has — a resolution and the slots of the extra bytes — so nothing about a saved fixture
 * changes shape.
 */

export const CHANNEL_LEVELS = ["Coarse", "Fine", "Ultra", "Extreme"] as const;
export type ChannelLevel = 0 | 1 | 2 | 3;

const RESOLUTIONS: ChannelResolution[] = ["u8", "u16", "u24", "u32"];

export type SlotRow = {
	slot: number;
	/** The channel this slot belongs to; for a Fine row, the coarse channel it refines. */
	channel: FixtureChannel;
	level: ChannelLevel;
};

export function slotRowKey(row: SlotRow) {
	return `${row.channel.id}:${row.level}`;
}

export function slotRows(mode: FixtureMode, split: number): SlotRow[] {
	const primary = derivePrimarySlots(mode).slots;
	const rows: SlotRow[] = [];
	for (const channel of mode.channels) {
		if (channel.split !== split) continue;
		rows.push({
			slot: primary.get(channel.id) ?? Number.MAX_SAFE_INTEGER,
			channel,
			level: 0,
		});
		channel.secondary_slots.forEach((slot, index) =>
			rows.push({ slot, channel, level: Math.min(index + 1, 3) as ChannelLevel }),
		);
	}
	return rows.sort((left, right) => left.slot - right.slot || left.level - right.level);
}

/** A raw value moved to another byte count the way a coarse/fine pair reads: 128 → 32768. */
function rescaleRaw(value: number, from: number, to: number, fill: boolean) {
	if (from === to) return value;
	if (to > from) {
		const factor = 2 ** (8 * (to - from));
		return value * factor + (fill ? factor - 1 : 0);
	}
	return Math.floor(value / 2 ** (8 * (from - to)));
}

function rescaleChannel(channel: FixtureChannel, bytes: number): FixtureChannel {
	const from = resolutionBytes(channel.resolution);
	const resolution = RESOLUTIONS[bytes - 1];
	if (from === bytes) return { ...channel, resolution };
	const full = maxRaw(channel.resolution);
	const raw = (value: number, fill = false) =>
		rescaleRaw(value, from, bytes, fill || value === full);
	return {
		...channel,
		resolution,
		default_raw: raw(channel.default_raw),
		highlight_raw: raw(channel.highlight_raw),
		functions: channel.functions.map((fn) => ({
			...fn,
			dmx_from: raw(fn.dmx_from),
			dmx_to: raw(fn.dmx_to, true),
			behavior:
				fn.behavior.type === "fixed" || fn.behavior.type === "indexed"
					? { ...fn.behavior, raw_value: raw(fn.behavior.raw_value) }
					: fn.behavior,
		})),
	};
}

/** What else in the mode stores raw values of a channel whose byte count changed. */
function rescaleReferences(
	mode: FixtureMode,
	before: Map<string, number>,
): FixtureMode {
	const changed = (id: string) => {
		const from = before.get(id);
		const channel = mode.channels.find((candidate) => candidate.id === id);
		if (!from || !channel) return null;
		const to = resolutionBytes(channel.resolution);
		return from === to ? null : { from, to, full: 2 ** (8 * from) - 1 };
	};
	const raw = (id: string, value: number, fill = false) => {
		const change = changed(id);
		return change
			? rescaleRaw(value, change.from, change.to, fill || value === change.full)
			: value;
	};
	const system = (value: ColorSystem): ColorSystem =>
		value.type === "discrete_wheel"
			? {
					...value,
					slots: value.slots.map((slot) => ({
						...slot,
						dmx_from: raw(value.channel_id, slot.dmx_from),
						dmx_to: raw(value.channel_id, slot.dmx_to, true),
					})),
				}
			: value;
	return {
		...mode,
		control_actions: mode.control_actions.map((action) => ({
			...action,
			assignments: action.assignments.map((assignment) => ({
				...assignment,
				active_raw: raw(assignment.channel_id, assignment.active_raw),
				inactive_raw: raw(assignment.channel_id, assignment.inactive_raw),
			})),
		})),
		color_systems: mode.color_systems.map((head) => ({
			...head,
			system: system(head.system),
		})),
	};
}

/**
 * Writes a split back from its rows.
 *
 * Coarse rows become the split's channels in slot order, which is what makes each land on its own
 * slot; every other row becomes a stored byte slot of its channel, in level order. A channel that
 * lost its coarse row is gone, along with what referred to it.
 */
function commitRows(mode: FixtureMode, split: number, rows: SlotRow[]): FixtureMode {
	const before = new Map(
		mode.channels.map((channel) => [channel.id, resolutionBytes(channel.resolution)]),
	);
	const coarse = rows
		.filter((row) => row.level === 0)
		.sort((left, right) => left.slot - right.slot);
	const kept = new Set(coarse.map((row) => row.channel.id));
	const bytes = new Map<string, SlotRow[]>();
	for (const row of rows) {
		if (row.level === 0 || !kept.has(row.channel.id)) continue;
		bytes.set(row.channel.id, [...(bytes.get(row.channel.id) ?? []), row]);
	}
	const written = coarse.map(({ channel }) => {
		const parts = (bytes.get(channel.id) ?? [])
			.sort((left, right) => left.level - right.level || left.slot - right.slot)
			.slice(0, 3);
		return {
			...rescaleChannel(channel, parts.length + 1),
			split,
			secondary_slots: parts.map((part) => part.slot),
		};
	});
	const removed = mode.channels.filter(
		(channel) => channel.split === split && !kept.has(channel.id),
	);
	const channels: FixtureChannel[] = [];
	let placed = false;
	for (const channel of mode.channels) {
		if (channel.split !== split) channels.push(channel);
		else if (!placed) {
			channels.push(...written);
			placed = true;
		}
	}
	if (!placed) channels.push(...written);
	const highest = Math.max(0, ...rows.map((row) => row.slot));
	let next: FixtureMode = {
		...mode,
		channels,
		splits: mode.splits.map((candidate) =>
			candidate.number === split
				? { ...candidate, footprint: Math.min(512, Math.max(candidate.footprint, highest)) }
				: candidate,
		),
	};
	for (const channel of removed) next = removeChannel(next, channel.id);
	return rescaleReferences(next, before);
}

/** A new coarse channel for a byte that stops refining another one. */
function channelFromByte(mode: FixtureMode, owner: FixtureChannel): FixtureChannel {
	return {
		...blankChannel(mode, owner.split),
		head_id: owner.head_id,
		attribute: owner.attribute,
		fixture_attribute: owner.fixture_attribute,
		canonical_transform: owner.canonical_transform,
		behavior: owner.behavior,
		physical_min: owner.physical_min,
		physical_max: owner.physical_max,
		unit: owner.unit,
		invert: owner.invert,
		snap: owner.snap,
		reacts_to_virtual_intensity: owner.reacts_to_virtual_intensity,
		virtual_intensity_inverted: owner.virtual_intensity_inverted ?? false,
		reacts_to_sequence_master: owner.reacts_to_sequence_master,
		reacts_to_group_master: owner.reacts_to_group_master,
		reacts_to_grand_master: owner.reacts_to_grand_master,
		default_raw: 0,
		highlight_raw: 0,
	};
}

export type LevelChange =
	| { mode: FixtureMode; error?: undefined }
	| { mode?: undefined; error: string };

/** Makes one row the given level, joining it to or parting it from a coarse channel. */
export function setRowLevel(
	mode: FixtureMode,
	split: number,
	target: SlotRow,
	level: ChannelLevel,
	attributeLabel: (attribute: string) => string = (attribute) => attribute,
): LevelChange {
	if (level === target.level) return { mode };
	const rows = slotRows(mode, split);
	const index = rows.findIndex((row) => slotRowKey(row) === slotRowKey(target));
	if (index < 0) return { mode };
	const row = rows[index];

	if (level === 0) {
		rows[index] = { ...row, channel: channelFromByte(mode, row.channel), level: 0 };
		return { mode: commitRows(mode, split, rows) };
	}

	if (row.level > 0) {
		// Swap with whichever byte of the same channel held the level, so levels stay one of each.
		const holder = rows.findIndex(
			(candidate) => candidate.channel.id === row.channel.id && candidate.level === level,
		);
		if (holder >= 0) rows[holder] = { ...rows[holder], level: row.level };
		rows[index] = { ...row, level };
		return { mode: commitRows(mode, split, rows) };
	}

	// A coarse row becomes a further byte of the nearest coarse channel doing the same job.
	const sameJob = (candidate: SlotRow) =>
		candidate.level === 0 &&
		candidate.channel.id !== row.channel.id &&
		candidate.channel.head_id === row.channel.head_id &&
		candidate.channel.behavior === row.channel.behavior &&
		candidate.channel.attribute === row.channel.attribute;
	const owner =
		[...rows.slice(0, index)].reverse().find(sameJob) ??
		rows.slice(index + 1).find(sameJob);
	if (!owner) {
		const name = attributeLabel(row.channel.attribute);
		return {
			error: `There is no coarse ${name} on this head for this slot to refine. Give another slot the ${name} attribute first.`,
		};
	}
	const existing = rows.filter(
		(candidate) => candidate.channel.id === owner.channel.id && candidate.level > 0,
	).length;
	if (existing >= 3)
		return { error: `${attributeLabel(row.channel.attribute)} already uses all four bytes.` };
	const joined = rows.map((candidate) =>
		candidate.channel.id === row.channel.id
			? {
					...candidate,
					channel: owner.channel,
					// The joining slot takes the level asked for; bytes it carried follow after.
					level: (candidate === row
						? Math.min(level, existing + 1)
						: Math.min(3, existing + 1 + candidate.level)) as ChannelLevel,
				}
			: candidate,
	);
	return { mode: commitRows(mode, split, joined) };
}

/** Removes one slot; a coarse row takes its channel's other bytes with it. Later slots close up. */
export function removeSlotRow(
	mode: FixtureMode,
	split: number,
	target: SlotRow,
): FixtureMode {
	const rows = slotRows(mode, split);
	const gone = rows.filter((row) =>
		target.level === 0
			? row.channel.id === target.channel.id
			: slotRowKey(row) === slotRowKey(target),
	);
	const goneSlots = gone.map((row) => row.slot);
	const rest = rows
		.filter((row) => !gone.includes(row))
		.map((row) => ({
			...row,
			slot: row.slot - goneSlots.filter((slot) => slot < row.slot).length,
		}));
	const footprint =
		mode.splits.find((candidate) => candidate.number === split)?.footprint ?? 1;
	const next = commitRows(
		{
			...mode,
			splits: mode.splits.map((candidate) =>
				candidate.number === split
					? { ...candidate, footprint: Math.max(1, footprint - gone.length) }
					: candidate,
			),
		},
		split,
		rest,
	);
	return next;
}

/**
 * Moves a channel, with every byte of it, to the end of another split.
 *
 * Its old slots close up behind it, and its bytes land on consecutive slots in the new split, so a
 * 16-bit channel arrives as a coarse and fine pair rather than as a coarse byte pointing at slots
 * numbered for the split it left.
 */
export function moveChannelToSplit(
	mode: FixtureMode,
	channel: FixtureChannel,
	split: number,
): FixtureMode {
	if (channel.split === split) return mode;
	const coarse = slotRows(mode, channel.split).find(
		(row) => row.channel.id === channel.id && row.level === 0,
	);
	if (!coarse) return mode;
	const lifted = removeSlotRow(mode, channel.split, coarse);
	const rows = slotRows(lifted, split);
	const first = Math.max(0, ...rows.map((row) => row.slot)) + 1;
	const extra = resolutionBytes(channel.resolution) - 1;
	const moved: FixtureChannel = {
		...channel,
		split,
		secondary_slots: Array.from({ length: extra }, (_, index) => first + index + 1),
	};
	return commitRows(lifted, split, [
		...rows,
		{ slot: first, channel: moved, level: 0 },
		...moved.secondary_slots.map((slot, index) => ({
			slot,
			channel: moved,
			level: (index + 1) as ChannelLevel,
		})),
	]);
}

/** Swaps a row with its neighbour in slot order. */
export function moveSlotRow(
	mode: FixtureMode,
	split: number,
	target: SlotRow,
	direction: -1 | 1,
): FixtureMode | null {
	const rows = slotRows(mode, split);
	const index = rows.findIndex((row) => slotRowKey(row) === slotRowKey(target));
	const peer = rows[index + direction];
	if (index < 0 || !peer) return null;
	const slot = rows[index].slot;
	rows[index] = { ...rows[index], slot: peer.slot };
	rows[index + direction] = { ...peer, slot };
	return commitRows(mode, split, rows);
}

/** Moves a row onto another row's slot, as a drag does, shifting the rows between. */
export function moveSlotRowTo(
	mode: FixtureMode,
	split: number,
	source: SlotRow,
	target: SlotRow,
): FixtureMode | null {
	const rows = slotRows(mode, split);
	const from = rows.findIndex((row) => slotRowKey(row) === slotRowKey(source));
	const to = rows.findIndex((row) => slotRowKey(row) === slotRowKey(target));
	if (from < 0 || to < 0 || from === to) return null;
	const slots = rows.map((row) => row.slot);
	const [moved] = rows.splice(from, 1);
	rows.splice(to, 0, moved);
	return commitRows(
		mode,
		split,
		rows.map((row, index) => ({ ...row, slot: slots[index] })),
	);
}
