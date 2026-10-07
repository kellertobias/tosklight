import type {
	OutputReadoutSnapshot,
	ProgrammingComponentDescriptor,
	ProgrammingScalarIntent,
} from "../../../../api/familyEncoderModels";
import { presentNumber } from "../familyValuePresentation";
import type { ProgrammerValueEntry } from "./familyEncoderDisplay";

/**
 * Position readout inputs shared by the Position encoders and the Position Special Dialog
 * (TL-652).
 *
 * - **Group values.** With a group selected the Programmer holds Position as one group value, not
 *   per fixture. The selected group's value is projected onto every member, newest (LTP) first,
 *   so Point/X/Y/Z read the active Target and the next X/Y/Z edit keeps its reference instead of
 *   re-activating Target at the Origin.
 * - **Frame request.** A fixture without a Programmer Position value reads the typed request of
 *   the displayed frame (a Cue or Preset), so a valid Target never reads as an em dash.
 * - **Ranges.** Differing numbers read as their real minimum...maximum in descriptor units,
 *   never an average and never a bare Mixed.
 * - **Provenance.** Angles resolved from a Target say so: From XYZ (Origin), From Point, or From
 *   Target when the selection aims at different references.
 */

export const FROM_XYZ_LABEL = "From XYZ";
export const FROM_POINT_LABEL = "From Point";
export const FROM_TARGET_LABEL = "From Target";

const POSITION = "position";

export interface ProgrammerGroupValueEntry {
	groupId: string;
	attribute: string;
	value: ProgrammerValueEntry["value"];
	programmerOrder?: number;
}

type OrderedEntry = ProgrammerValueEntry & { programmerOrder?: number };

/**
 * The selection's programmed Position per fixture: fixture values plus the selected group's
 * value projected onto its members (`fixtureIds`), the newest programmer order winning.
 */
export function selectionPositionEntries(input: {
	fixtureValues: readonly OrderedEntry[];
	groupValues: readonly ProgrammerGroupValueEntry[];
	groupId: string | null;
	fixtureIds: readonly string[];
}): ProgrammerValueEntry[] {
	const group = input.groupId
		? input.groupValues.find(
				(entry) => entry.groupId === input.groupId && entry.attribute === POSITION,
			)
		: undefined;
	if (!group) return input.fixtureValues as ProgrammerValueEntry[];
	const members = new Set(input.fixtureIds);
	const order = group.programmerOrder ?? Number.NEGATIVE_INFINITY;
	const covered = new Set<string>();
	const kept = input.fixtureValues.filter((entry) => {
		if (entry.attribute !== POSITION || !members.has(entry.fixtureId)) return true;
		const newer = (entry.programmerOrder ?? Number.NEGATIVE_INFINITY) > order;
		if (newer) covered.add(entry.fixtureId);
		return newer;
	});
	const projected = input.fixtureIds
		.filter((id) => !covered.has(id))
		.map((fixtureId) => ({ fixtureId, attribute: POSITION, value: group.value }));
	return [...kept, ...projected];
}

/** Adds the displayed frame's requested Position for fixtures the Programmer does not hold. */
export function withFrameRequests(
	entries: readonly ProgrammerValueEntry[],
	readouts: OutputReadoutSnapshot | null,
	fixtureIds: readonly string[],
): readonly ProgrammerValueEntry[] {
	if (!readouts?.owners.length) return entries;
	const held = new Set(
		entries.filter((entry) => entry.attribute === POSITION).map((entry) => entry.fixtureId),
	);
	const added: ProgrammerValueEntry[] = [];
	for (const id of fixtureIds) {
		if (held.has(id)) continue;
		const requested = readouts.owners.find((owner) => owner.fixture_id === id)?.requested;
		if (requested?.kind === "position")
			added.push({ fixtureId: id, attribute: POSITION, value: requested });
	}
	return added.length ? [...entries, ...added] : entries;
}

/** The target provenance label when every requested Position of the fixtures is a Target. */
export function positionTargetProvenance(
	entries: readonly ProgrammerValueEntry[],
	fixtureIds: readonly string[],
): string | null {
	const wanted = new Set(fixtureIds);
	const intents = entries.flatMap((entry) =>
		entry.attribute === POSITION && wanted.has(entry.fixtureId) && entry.value.kind === "position"
			? [entry.value.value]
			: [],
	);
	if (!intents.length || intents.some((intent) => intent.kind !== "target")) return null;
	const kinds = new Set(
		intents.map((intent) => (intent.kind === "target" ? intent.reference.kind : "")),
	);
	if (kinds.size !== 1) return FROM_TARGET_LABEL;
	if (kinds.has("origin")) return FROM_XYZ_LABEL;
	const points = new Set(
		intents.map((intent) =>
			intent.kind === "target" && intent.reference.kind === "point"
				? intent.reference.point_id
				: "",
		),
	);
	return points.size === 1 ? FROM_POINT_LABEL : FROM_TARGET_LABEL;
}

/**
 * `min...max` of differing numbers in the descriptor's display unit, or the one shared text when
 * they agree at display resolution. Null for an empty list.
 */
export function presentNumericRange(
	values: readonly number[],
	descriptor: ProgrammingComponentDescriptor,
): string | null {
	const finite = values.filter(Number.isFinite);
	if (!finite.length) return null;
	const minimum = presentNumber(Math.min(...finite), descriptor).text;
	const maximum = presentNumber(Math.max(...finite), descriptor).text;
	return minimum === maximum ? minimum : `${minimum}...${maximum}`;
}

/** The plain numbers of scalar intents, or null when one of them is a spread. */
export function scalarValues(intents: readonly ProgrammingScalarIntent[]) {
	const values: number[] = [];
	for (const intent of intents) {
		if (intent.kind !== "value") return null;
		values.push(intent.value);
	}
	return values;
}
