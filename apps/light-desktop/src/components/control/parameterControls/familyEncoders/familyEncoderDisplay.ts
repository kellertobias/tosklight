import type {
	FamilyEncoderComponentSlot,
	OutputReadoutSnapshot,
	ProgrammingAttributeValue,
	ProgrammingScalarIntent,
	ProgrammingTargetReference,
} from "../../../../api/familyEncoderModels";
import type { PositionRepresentation } from "../../../../features/programmerValues/positionGestureSession";
import {
	MIXED_LABEL,
	presentNumber,
	presentScalarSelection,
} from "../familyValuePresentation";
import {
	positionTargetProvenance,
	presentNumericRange,
	scalarValues,
	withFrameRequests,
} from "./positionReadouts";

/**
 * Encoder readouts of semantic slots (TL-549/550/551 UI foundation).
 *
 * - Pan/Tilt read the resolved commanded angles of the displayed source (TL-594 readouts) and
 *   say so, naming a Target as their provenance (TL-652: From XYZ / From Point); divergent
 *   copies read their real minimum...maximum, never an average. Without a readout they fall
 *   back to the requested Angles, labelled requested.
 * - Position components without a Programmer value read the displayed frame's request (TL-652).
 * - Every other component reads its requested value through the TL-619 presentation helper.
 * - Nothing here refits, normalizes or guesses; an unknown value reads as an em dash.
 */

export interface ProgrammerValueEntry {
	fixtureId: string;
	attribute: string;
	value: ProgrammingAttributeValue;
}

export interface FamilySlotDisplay {
	/** The dial position in descriptor units, when one requested/resolved value is shared. */
	value: number | null;
	text: string;
	source: "resolved" | "requested" | "none";
	/** TL-652: why resolved angles read as they do, e.g. From XYZ; absent means Resolved. */
	provenance?: string;
	/**
	 * TL-637: set only on a Position slot whose fixtures have no Position physical data, so
	 * nothing can seed an edit. The encoder and dialog show a quiet unsupported state instead.
	 */
	unsupported?: true;
}

const NONE: FamilySlotDisplay = { value: null, text: "—", source: "none" };

function ownerAttribute(slot: FamilyEncoderComponentSlot) {
	return slot.descriptor.owner;
}

/** The requested scalar of one component in one programmed value, if it carries one. */
export function requestedComponentIntent(
	slot: FamilyEncoderComponentSlot,
	value: ProgrammingAttributeValue,
): ProgrammingScalarIntent | null {
	const component = slot.component;
	if (value.kind === "position") {
		const intent = value.value;
		if (intent.kind === "angles") {
			if (component.kind === "pan") return intent.pan_degrees;
			if (component.kind === "tilt") return intent.tilt_degrees;
			return null;
		}
		const axis = { target_x: 0, target_y: 1, target_z: 2 } as Record<string, number>;
		const index = axis[component.kind];
		return index === undefined ? null : intent.offset_metres[index] ?? null;
	}
	if (value.kind === "zoom")
		return component.kind === "zoom" ? value.value.opening_degrees : null;
	if (value.kind === "normalized" && component.kind === "focus")
		return { kind: "value", value: value.value };
	if (value.kind === "color_program" && value.value.kind === "semantic") {
		const intent = value.value.intent;
		if (component.kind !== "color") return null;
		const scalar = (number: number): ProgrammingScalarIntent => ({
			kind: "value",
			value: number,
		});
		switch (component.component) {
			case "red":
				return scalar(intent.recipe.rgb[0]);
			case "green":
				return scalar(intent.recipe.rgb[1]);
			case "blue":
				return scalar(intent.recipe.rgb[2]);
			case "amber":
				return scalar(intent.recipe.amber);
			case "white_blend":
				return scalar(intent.white_blend);
			case "temperature":
				return scalar(intent.white_target.kelvin);
			case "duv":
				return scalar(intent.white_target.duv);
			case "uv":
				return scalar(intent.uv.amount);
			default:
				return null;
		}
	}
	return null;
}

export function requestedSlotIntents(
	slot: FamilyEncoderComponentSlot,
	values: readonly ProgrammerValueEntry[],
) {
	const owner = ownerAttribute(slot);
	const fixtures = new Set(slot.fixture_ids);
	return values.flatMap((entry) => {
		if (entry.attribute !== owner || !fixtures.has(entry.fixtureId)) return [];
		const intent = requestedComponentIntent(slot, entry.value);
		return intent ? [intent] : [];
	});
}

function resolvedAngles(
	slot: FamilyEncoderComponentSlot,
	readouts: OutputReadoutSnapshot | null,
	programmerValues: readonly ProgrammerValueEntry[],
): FamilySlotDisplay | null {
	const axis = slot.component.kind;
	if ((axis !== "pan" && axis !== "tilt") || !readouts) return null;
	const owners = slot.fixture_ids.flatMap((id) => {
		const owner = readouts.owners.find((entry) => entry.fixture_id === id);
		return owner?.position.available ? [owner.position] : [];
	});
	if (!owners.length) return null;
	const pick = (angles: { pan_degrees: number; tilt_degrees: number }) =>
		axis === "pan" ? angles.pan_degrees : angles.tilt_degrees;
	// A divergent owner contributes every copy's commanded angle to the range.
	const values = owners.flatMap((position) =>
		position.common ? [pick(position.common)] : position.commands.map(pick),
	);
	const provenance = positionTargetProvenance(
		withFrameRequests(programmerValues, readouts, slot.fixture_ids),
		slot.fixture_ids,
	);
	const tagged = provenance ? { provenance } : {};
	const first = values[0];
	const shared =
		first !== undefined &&
		owners.every((position) => position.common) &&
		values.every((value) => value === first);
	if (shared)
		return {
			value: first,
			text: presentNumber(first, slot.descriptor).text,
			source: "resolved",
			...tagged,
		};
	// An owner that reports neither a common pose nor its copies cannot bound the range.
	const complete = owners.every((position) => position.common || position.commands.length);
	return {
		value: null,
		text: (complete && presentNumericRange(values, slot.descriptor)) || MIXED_LABEL,
		source: "resolved",
		...tagged,
	};
}

/**
 * TL-637: a Position slot (Pan, Tilt, Point, X/Y/Z) is unsupported when the displayed source is
 * valid (an accepted frame with a lease) yet reports no commanded pose for any of the slot's
 * fixtures and none of them has a requested Position value. Those profiles carry no Position
 * physical graph, so the server could only answer a first edit with a silent `no_change`.
 * Unknown or unclaimed readouts never count as unsupported, and a requested value keeps the slot
 * editable.
 */
export function positionSlotUnsupported(
	slot: FamilyEncoderComponentSlot,
	input: {
		programmerValues: readonly ProgrammerValueEntry[];
		readouts: OutputReadoutSnapshot | null;
	},
) {
	const readouts = input.readouts;
	if (slot.descriptor.owner !== "position" || !slot.fixture_ids.length) return false;
	if (!readouts || readouts.lease == null || readouts.unavailable) return false;
	const wanted = new Set(slot.fixture_ids);
	if (input.programmerValues.some((entry) => entry.attribute === "position" && wanted.has(entry.fixtureId)))
		return false;
	return slot.fixture_ids.every((id) => {
		const owner = readouts.owners.find((entry) => entry.fixture_id === id);
		return owner !== undefined && !owner.position.available && owner.requested == null;
	});
}

export function familySlotDisplay(
	slot: FamilyEncoderComponentSlot,
	input: {
		programmerValues: readonly ProgrammerValueEntry[];
		readouts: OutputReadoutSnapshot | null;
	},
): FamilySlotDisplay {
	const resolved = resolvedAngles(slot, input.readouts, input.programmerValues);
	if (resolved) return resolved;
	const position = slot.descriptor.owner === "position";
	const intents = requestedSlotIntents(
		slot,
		position
			? withFrameRequests(input.programmerValues, input.readouts, slot.fixture_ids)
			: input.programmerValues,
	);
	const presented = presentScalarSelection(intents, slot.descriptor);
	if (!presented)
		return positionSlotUnsupported(slot, input) ? { ...NONE, unsupported: true } : NONE;
	if (presented.kind === "mixed" && position) {
		const values = scalarValues(intents);
		const range = values ? presentNumericRange(values, slot.descriptor) : null;
		if (range) return { value: null, text: range, source: "requested" };
	}
	return {
		value:
			presented.kind === "value" ? presented.value.requested : null,
		text: presented.text,
		source: "requested",
	};
}

/** The selection's common Position representation and Target reference, when one exists. */
export function positionSelectionState(
	values: readonly ProgrammerValueEntry[],
	fixtureIds: readonly string[],
): {
	representation: PositionRepresentation | undefined;
	reference: ProgrammingTargetReference | undefined;
} {
	const wanted = new Set(fixtureIds);
	const intents = values.flatMap((entry) =>
		entry.attribute === "position" &&
		wanted.has(entry.fixtureId) &&
		entry.value.kind === "position"
			? [entry.value.value]
			: [],
	);
	if (!intents.length || intents.length !== wanted.size)
		return { representation: undefined, reference: undefined };
	const kinds = new Set(intents.map((intent) => intent.kind));
	if (kinds.size !== 1) return { representation: undefined, reference: undefined };
	const first = intents[0];
	if (first?.kind !== "target") return { representation: "angles", reference: undefined };
	const reference = first.reference;
	const shared = intents.every(
		(intent) =>
			intent.kind === "target" &&
			JSON.stringify(intent.reference) === JSON.stringify(reference),
	);
	return { representation: "target", reference: shared ? reference : undefined };
}
