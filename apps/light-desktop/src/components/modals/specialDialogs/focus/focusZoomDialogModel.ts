import type {
	FamilyEncoderComponentSlot,
	FamilyEncoderPagesSnapshot,
} from "../../../../api/familyEncoderModels";
import {
	familySlotDisplay,
	type ProgrammerValueEntry,
} from "../../../control/parameterControls/familyEncoders/familyEncoderDisplay";
import type {
	FocusZoomFocusValue,
	FocusZoomZoomValue,
} from "../intention/FocusZoomDialog";

/**
 * Pure model of the production Focus Special Dialog (TL-551).
 *
 * Every range, step and convention comes from the compiled descriptor and the selection limits the
 * family pages route publishes (`GET /api/v2/programming/family-encoder-pages`). Nothing here
 * converts degrees to DMX or inverts a calibration curve: Zoom stays a full opening angle in the
 * published beam/field convention and Focus stays the normalized 0–1 lens travel. Values shown
 * are the operator's requested values; no achieved Zoom readout is published yet, so achieved is
 * reported as unavailable instead of guessed.
 */

export interface FocusZoomSlots {
	focus: FamilyEncoderComponentSlot | null;
	zoom: FamilyEncoderComponentSlot | null;
}

const NO_SLOTS: FocusZoomSlots = { focus: null, zoom: null };

/** The Focus family's Focus and Zoom component slots, from any page, or nothing. */
export function focusZoomSlots(
	snapshot: FamilyEncoderPagesSnapshot | null,
): FocusZoomSlots {
	if (!snapshot?.semantic) return NO_SLOTS;
	const group = snapshot.families.find((entry) => entry.family === "focus");
	const slots = (group?.pages ?? []).flatMap((page) => page.slots);
	const find = (kind: "focus" | "zoom") => {
		const slot = slots.find(
			(entry) => entry?.kind === "component" && entry.component.kind === kind,
		);
		return slot?.kind === "component" && slot.edit === "scalar" ? slot : null;
	};
	return { focus: find("focus"), zoom: find("zoom") };
}

/** The selection's requested value in descriptor units; `null` with a label when none is shared. */
export interface RequestedControlValue {
	value: number | null;
	/** "Mixed", a spread, or "—" when nothing is requested. */
	text: string;
}

export function requestedControlValue(
	slot: FamilyEncoderComponentSlot | null,
	programmerValues: readonly ProgrammerValueEntry[],
): RequestedControlValue {
	if (!slot) return { value: null, text: "—" };
	const display = familySlotDisplay(slot, { programmerValues, readouts: null });
	return { value: display.value, text: display.text };
}

function domainBounds(slot: FamilyEncoderComponentSlot) {
	const domain = slot.descriptor.domain;
	return domain && domain.kind !== "finite" ? domain.bounds : null;
}

const finiteStep = (value: number, fallback: number) =>
	Number.isFinite(value) && value > 0 ? value : fallback;

/** Zoom limits: the selection's common published range, else the descriptor's bounded domain. */
export function zoomLimits(slot: FamilyEncoderComponentSlot | null) {
	const bounds = (slot && (slot.limits ?? domainBounds(slot))) ?? {
		min: 0,
		max: 180,
	};
	const step = finiteStep(slot?.descriptor.step ?? 1, 1);
	return {
		minimum: bounds.min,
		maximum: bounds.max,
		// Pointer motion resolves to the fine step; keys move one descriptor step, coarse ten.
		step: finiteStep(slot?.descriptor.fine_step ?? step / 10, step / 10),
		keyStep: step,
		largeKeyStep: step * 10,
	};
}

export function focusLimits(slot: FamilyEncoderComponentSlot | null) {
	const bounds = (slot && (slot.limits ?? domainBounds(slot))) ?? {
		min: 0,
		max: 1,
	};
	const step = finiteStep(slot?.descriptor.step ?? 0.01, 0.01);
	return {
		minimum: bounds.min,
		maximum: bounds.max,
		step,
		keyStep: step,
		largeKeyStep: step * 10,
	};
}

/** Quiet statuses; never an error, never blocking. */
export const ZOOM_UNSUPPORTED_STATUS = "Requested · unsupported";
export const ZOOM_REQUESTED_STATUS = "Requested · achieved not reported";
export const NOT_AVAILABLE_STATUS = "Not available";
export const NOT_PROGRAMMED_STATUS = "Not programmed";

/** "Mixed", a spread, or Not programmed, for a selection without one shared requested value. */
const unsharedStatus = (requested: RequestedControlValue) =>
	requested.text === "—" ? NOT_PROGRAMMED_STATUS : requested.text;

export interface ZoomDialogInput {
	slot: FamilyEncoderComponentSlot | null;
	requested: RequestedControlValue;
	/** The operator's latest dialog request, kept while the server has not reflected it. */
	local: number | undefined;
	/** The server refused or held a Zoom edit of this dialog. */
	refused: boolean;
}

export function zoomDialogValue(input: ZoomDialogInput): {
	zoom: FocusZoomZoomValue;
	status: string;
} {
	const limits = zoomLimits(input.slot);
	const convention = input.slot?.convention ?? null;
	const value = input.local ?? input.requested.value ?? limits.minimum;
	const zoom = { ...limits, value, convention };
	if (!input.slot) return { zoom, status: NOT_AVAILABLE_STATUS };
	if (convention === null || input.refused)
		return { zoom, status: ZOOM_UNSUPPORTED_STATUS };
	if (input.local === undefined && input.requested.value === null)
		return { zoom, status: unsharedStatus(input.requested) };
	return { zoom, status: ZOOM_REQUESTED_STATUS };
}

export function focusDialogValue(input: {
	slot: FamilyEncoderComponentSlot | null;
	requested: RequestedControlValue;
	local: number | undefined;
}): { focus: FocusZoomFocusValue; status: string | undefined } {
	const limits = focusLimits(input.slot);
	const value = input.local ?? input.requested.value ?? limits.minimum;
	const focus = { ...limits, value };
	if (!input.slot) return { focus, status: NOT_AVAILABLE_STATUS };
	if (input.local === undefined && input.requested.value === null)
		return { focus, status: unsharedStatus(input.requested) };
	return { focus, status: undefined };
}
