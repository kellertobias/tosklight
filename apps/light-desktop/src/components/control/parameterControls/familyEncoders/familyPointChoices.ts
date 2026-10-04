import type {
	FamilyEncoderComponentSlot,
	ProgrammingTargetReference,
} from "../../../../api/familyEncoderModels";
import type { PatchedFixture } from "../../../../api/types";
import {
	POSITION_TARGET_ORIGIN,
	positionTargetPoint,
} from "../../../../features/programmerValues/positionGestureSession";
import {
	positionPointLabel,
	positionPoints,
} from "../../../setup/fixturePatch/positionReference";
import { MIXED_LABEL, ORIGIN_LABEL } from "../familyValuePresentation";
import {
	type FamilySlotDisplay,
	type ProgrammerValueEntry,
	positionSelectionState,
} from "./familyEncoderDisplay";

/**
 * The Point slot's ordered choices (TL-544 G4): Origin, then every 3D Point of the show in Patch
 * table order, named as the Patch names them wherever an operator picks one. Hardware/OSC
 * detents, software encoder steps and the value-pad picker all walk or pick from this one list.
 */

export const MISSING_POINT_LABEL = "Missing point";

export interface FamilyPointChoice {
	/** Stable picker value: `origin` or `point:<fixture id>`. */
	value: string;
	label: string;
	reference: ProgrammingTargetReference;
}

export function pointChoiceValue(reference: ProgrammingTargetReference) {
	return reference.kind === "origin" ? "origin" : `point:${reference.point_id}`;
}

export function familyPointChoices(
	fixtures: readonly PatchedFixture[],
): FamilyPointChoice[] {
	return [
		{ value: "origin", label: ORIGIN_LABEL, reference: POSITION_TARGET_ORIGIN },
		...positionPoints(fixtures).map((point) => {
			const reference = positionTargetPoint(point.fixture_id);
			return { value: pointChoiceValue(reference), label: positionPointLabel(point), reference };
		}),
	];
}

/** The Point slot readout: the selection's shared reference, Mixed, or an em dash. */
export function pointSlotDisplay(
	slot: FamilyEncoderComponentSlot,
	values: readonly ProgrammerValueEntry[],
	choices: readonly FamilyPointChoice[],
): FamilySlotDisplay {
	const state = positionSelectionState(values, slot.fixture_ids);
	if (state.representation !== "target")
		return { value: null, text: "—", source: "none" };
	if (!state.reference) return { value: null, text: MIXED_LABEL, source: "requested" };
	const value = pointChoiceValue(state.reference);
	const choice = choices.find((entry) => entry.value === value);
	return {
		value: null,
		text: choice?.label ?? MISSING_POINT_LABEL,
		source: "requested",
	};
}
