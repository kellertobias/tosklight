import type {
	FamilyEncoderComponentSlot,
	OutputReadoutSnapshot,
	ProgrammingAttributeValue,
	ProgrammingColorComponent,
	ProgrammingScalarIntent,
} from "../../../../api/familyEncoderModels";
import { MIXED_LABEL, presentScalarSelection } from "../familyValuePresentation";
import {
	type FamilySlotDisplay,
	type ProgrammerValueEntry,
	requestedComponentIntent,
} from "./familyEncoderDisplay";

/**
 * Readouts of the semantic Color encoder slots, pages 1 and 2 (TL-653).
 *
 * - A fixture's Programmer Color value is read through the TL-619 presentation helper; with a
 *   group selected, the group's Color value stands for every member that holds no newer value of
 *   its own (LTP, as the Programmer resolves it).
 * - A fixture the Programmer holds no Color for reads the Color its displayed frame resolves
 *   (TL-657: a running Cue or Playback), exactly the colour a first semantic Color edit starts
 *   from. Only a fixture whose output holds no colour reads the open-white start: the semantic
 *   default intent and the colour the fixture shows at rest. It never reads as a dash, and black
 *   or zero stay real values.
 * - A fixture holding a Direct colour reads `Direct`: its semantic value is known only once a
 *   page 1/2 edit adopts it. Differing values read Mixed.
 */

/** A semantic Color slot whose fixtures hold a Direct colour. */
export const DIRECT_COLOR_LABEL = "Direct";

/**
 * The open-white start of a first semantic Color edit on a fixture that holds no colour: the
 * semantic default intent (full RGB, no Amber, White Blend 0, 6500 K, Duv 0, no UV), exactly what
 * the desk seeds such an edit with. `null`: the component has no value of its own (Hue of white).
 */
export const OPEN_WHITE_START: Readonly<Record<ProgrammingColorComponent, number | null>> = {
	red: 1,
	green: 1,
	blue: 1,
	amber: 0,
	hue: null,
	saturation: 0,
	white_blend: 0,
	temperature: 6500,
	duv: 0,
	uv: 0,
	relative_output: 1,
};

/**
 * A semantic Color reading. `start` marks the open-white start of fixtures that hold no Color
 * (requested by nothing yet); it is presented like a requested value.
 */
export type ColorSlotDisplay = FamilySlotDisplay & { start?: true };

/** One value of the selected group (the Programmer's group values). */
export interface ColorGroupValueEntry {
	groupId: string;
	attribute: string;
	value: ProgrammingAttributeValue;
	programmerOrder?: number;
}

type OrderedEntry = ProgrammerValueEntry & { programmerOrder?: number };

const COLOR = "color";

/** Each slot fixture's held Color value: its own, or the selected group's newer one. */
function heldColor(
	fixtureIds: readonly string[],
	values: readonly OrderedEntry[],
	group: ColorGroupValueEntry | undefined,
) {
	const held = new Map<string, ProgrammingAttributeValue>();
	const order = group?.programmerOrder ?? Number.NEGATIVE_INFINITY;
	for (const entry of values) {
		if (entry.attribute !== COLOR) continue;
		if (group && (entry.programmerOrder ?? Number.NEGATIVE_INFINITY) <= order) continue;
		held.set(entry.fixtureId, entry.value);
	}
	if (group)
		for (const fixtureId of fixtureIds)
			if (!held.has(fixtureId)) held.set(fixtureId, group.value);
	return held;
}

/**
 * TL-657: adds the displayed frame's Color for every fixture of `fixtureIds` that `entries`
 * holds no Color for, so a Cue or Playback colour reads as the start a first edit adopts.
 */
export function withFrameColors<T extends ProgrammerValueEntry>(
	entries: readonly T[],
	readouts: OutputReadoutSnapshot | null | undefined,
	fixtureIds: readonly string[],
): readonly (T | ProgrammerValueEntry)[] {
	const added = frameColors(readouts, fixtureIds, (id) =>
		entries.some((entry) => entry.attribute === COLOR && entry.fixtureId === id),
	);
	return added.length ? [...entries, ...added] : entries;
}

function frameColors(
	readouts: OutputReadoutSnapshot | null | undefined,
	fixtureIds: readonly string[],
	held: (fixtureId: string) => boolean,
): ProgrammerValueEntry[] {
	if (!readouts?.owners.length || readouts.unavailable) return [];
	const added: ProgrammerValueEntry[] = [];
	for (const fixtureId of fixtureIds) {
		if (held(fixtureId)) continue;
		const color = readouts.owners.find((owner) => owner.fixture_id === fixtureId)?.color;
		if (color?.kind === "color_program")
			added.push({ fixtureId, attribute: COLOR, value: color });
	}
	return added;
}

/**
 * The reading of a semantic Color component over the slot's fixtures; `null` when `slot` is not
 * a semantic Color component (Wheels, Direct and other families keep their own readouts).
 */
export function colorSlotDisplay(
	slot: FamilyEncoderComponentSlot,
	input: {
		programmerValues: readonly OrderedEntry[];
		groupValues?: readonly ColorGroupValueEntry[];
		groupId?: string | null;
		/** The displayed frame's readouts of the slot's fixtures (TL-657), when read. */
		readouts?: OutputReadoutSnapshot | null;
	},
): ColorSlotDisplay | null {
	const component = slot.component;
	if (component.kind !== "color" || slot.descriptor.owner !== COLOR) return null;
	const group = input.groupId
		? input.groupValues?.find(
				(entry) => entry.groupId === input.groupId && entry.attribute === COLOR,
			)
		: undefined;
	const held = heldColor(slot.fixture_ids, input.programmerValues, group);
	for (const entry of frameColors(input.readouts, slot.fixture_ids, (id) => held.has(id)))
		held.set(entry.fixtureId, entry.value);
	const start = OPEN_WHITE_START[component.component];
	const intents: ProgrammingScalarIntent[] = [];
	let direct = 0;
	let started = 0;
	for (const fixtureId of slot.fixture_ids) {
		const value = held.get(fixtureId);
		if (value?.kind === "color_program" && value.value.kind === "direct") {
			direct += 1;
			continue;
		}
		const intent = value ? requestedComponentIntent(slot, value) : null;
		if (intent) intents.push(intent);
		else if (!value && start !== null) {
			started += 1;
			intents.push({ kind: "value", value: start });
		}
	}
	if (direct)
		return {
			value: null,
			text: intents.length ? MIXED_LABEL : DIRECT_COLOR_LABEL,
			source: "requested",
		};
	const presented = presentScalarSelection(intents, slot.descriptor);
	if (!presented) return { value: null, text: "—", source: "none" };
	return {
		value: presented.kind === "value" ? presented.value.requested : null,
		text: presented.text,
		source: "requested",
		...(started === intents.length ? { start: true as const } : {}),
	};
}
