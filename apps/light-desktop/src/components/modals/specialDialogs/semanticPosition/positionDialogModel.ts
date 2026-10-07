import type {
	FamilyEncoderComponentSlot,
	FamilyEncoderGroup,
	FamilyEncoderPagesSnapshot,
	OutputReadoutSnapshot,
} from "../../../../api/familyEncoderModels";
import {
	familySlotDisplay,
	type ProgrammerValueEntry,
} from "../../../control/parameterControls/familyEncoders/familyEncoderDisplay";
import type { PositionRepresentation } from "../../../../features/programmerValues/positionGestureSession";
import type {
	PositionJoystickRates,
	PositionPanValue,
	PositionTiltValue,
} from "../intention/PositionDialog";

/**
 * Pure model of the production Position Special Dialog (TL-549).
 *
 * - **Values.** Each angle reads through the same `familySlotDisplay` the Position encoders use
 *   (hardware/software parity). While Target is active, or before any semantic Position value
 *   exists, the angles are the resolved commanded angles of the displayed output (TL-594
 *   readouts) and are captioned `Resolved`. While Angles are active the requested angles are
 *   shown: a Preload readout is read once per claim and would otherwise go stale under edits.
 * - **Ranges.** A selection whose copies diverge reads its minimum...maximum in the encoders
 *   (TL-652). Nothing is averaged or guessed: the dialog then edits that axis relatively, from a
 *   virtual zero (`Relative`).
 * - **Provenance.** Angles resolved from a Target are captioned with it (`From XYZ`, `From
 *   Point`, `From Target`) instead of `Resolved` (TL-652).
 * - **Unavailable.** Without a slot, a readout or a requested value the axis is inert (`NaN`).
 * - **Unsupported (TL-637).** When the displayed output reports no pose for any fixture and
 *   nothing is requested, the profiles have no Position physical data: the axes stay inert, the
 *   caption reads `Unsupported` and no gesture is sent, instead of a silent `no_change`.
 * - **Limits.** The route publishes no Pan/Tilt programming limits yet (`limits_source:
 *   unknown`). The dialog then uses the approved example window, −720…720° Pan and −135…135°
 *   Tilt, widened to include the current value. This is an interaction window only; the
 *   resolver still applies each fixture's physical range.
 * - **Rates.** `ProgrammingComponentDescriptor` carries no rate. Until it does, the joystick uses
 *   the approved interaction defaults, 120°/s Pan and 90°/s Tilt at full deflection.
 */

export const POSITION_DIALOG_DEFAULT_RATES: PositionJoystickRates = {
	panDegreesPerSecond: 120,
	tiltDegreesPerSecond: 90,
};

export const POSITION_DIALOG_DEFAULT_LIMITS = {
	pan: { min: -720, max: 720 },
	tilt: { min: -135, max: 135 },
} as const;

export const RESOLVED_CAPTION = "Resolved";
export const RELATIVE_CAPTION = "Relative";
/** TL-637: the selection's fixtures have no Position physical data (quiet, never an alert). */
export const UNSUPPORTED_CAPTION = "Unsupported";

export type PositionAxis = "pan" | "tilt";
export type PositionAxisMode = "absolute" | "relative" | "unavailable";

export interface PositionAxisModel {
	mode: PositionAxisMode;
	/** Degrees in absolute mode, the virtual zero in relative mode, NaN when unavailable. */
	value: number;
	source: "resolved" | "requested" | "none";
	/** TL-652: the Target the resolved angles come from, e.g. From XYZ. */
	provenance?: string;
	limits: { min: number; max: number };
	step: number;
	keyStep: number;
	/** TL-637: no Position physical data for this selection (see `positionSlotUnsupported`). */
	unsupported?: true;
}

export function positionFamilyGroup(
	snapshot: FamilyEncoderPagesSnapshot | null,
): FamilyEncoderGroup | null {
	if (!snapshot?.semantic) return null;
	return snapshot.families.find((group) => group.family === "position") ?? null;
}

/** The Pan or Tilt component slot of the published Position pages. */
export function positionAxisSlot(
	group: FamilyEncoderGroup | null,
	axis: PositionAxis,
): FamilyEncoderComponentSlot | null {
	for (const page of group?.pages ?? [])
		for (const slot of page.slots)
			if (slot?.kind === "component" && slot.component.kind === axis)
				return slot;
	return null;
}

function finiteLimits(slot: FamilyEncoderComponentSlot, axis: PositionAxis) {
	const limits = slot.limits;
	if (limits && Number.isFinite(limits.min) && Number.isFinite(limits.max))
		return { min: limits.min, max: limits.max };
	return POSITION_DIALOG_DEFAULT_LIMITS[axis];
}

export function positionAxisModel(
	slot: FamilyEncoderComponentSlot | null,
	axis: PositionAxis,
	input: {
		programmerValues: readonly ProgrammerValueEntry[];
		readouts: OutputReadoutSnapshot | null;
		representation: PositionRepresentation | undefined;
	},
): PositionAxisModel {
	if (!slot)
		return {
			mode: "unavailable",
			value: Number.NaN,
			source: "none",
			limits: POSITION_DIALOG_DEFAULT_LIMITS[axis],
			step: 0.1,
			keyStep: 1,
		};
	const display = familySlotDisplay(slot, {
		programmerValues: input.programmerValues,
		readouts: input.representation === "angles" ? null : input.readouts,
	});
	const limits = finiteLimits(slot, axis);
	const step = slot.descriptor.fine_step > 0 ? slot.descriptor.fine_step : slot.descriptor.step;
	const keyStep = slot.descriptor.step > 0 ? slot.descriptor.step : step;
	const provenance = display.provenance ? { provenance: display.provenance } : {};
	if (display.value !== null)
		return {
			mode: "absolute",
			value: display.value,
			source: display.source,
			...provenance,
			limits: {
				min: Math.min(limits.min, display.value),
				max: Math.max(limits.max, display.value),
			},
			step,
			keyStep,
		};
	if (display.source !== "none")
		return { mode: "relative", value: 0, source: display.source, ...provenance, limits, step, keyStep };
	return {
		mode: "unavailable",
		value: Number.NaN,
		source: "none",
		limits,
		step,
		keyStep,
		...(display.unsupported ? { unsupported: true as const } : {}),
	};
}

/** TL-637: either axis has no Position physical data, so the dialog sends nothing. */
export function positionDialogUnsupported(pan: PositionAxisModel, tilt: PositionAxisModel) {
	return pan.unsupported === true || tilt.unsupported === true;
}

/**
 * The caption shown with the values: `Unsupported` without Position physical data, `Relative`
 * for an axis whose fixtures differ (with the Target provenance, TL-652), else `Resolved` — or
 * the Target provenance such as `From XYZ` — when read back.
 */
export function positionValueCaption(
	pan: PositionAxisModel,
	tilt: PositionAxisModel,
): string | undefined {
	if (positionDialogUnsupported(pan, tilt)) return UNSUPPORTED_CAPTION;
	const provenance = pan.provenance ?? tilt.provenance;
	if (pan.mode === "relative" || tilt.mode === "relative")
		return provenance ? `${RELATIVE_CAPTION} · ${provenance}` : RELATIVE_CAPTION;
	if (pan.source === "resolved" || tilt.source === "resolved") return provenance ?? RESOLVED_CAPTION;
	return undefined;
}

/** Full-deflection rates. Descriptors publish no rate yet, so the interaction defaults apply. */
export function positionJoystickRates(
	_pan: FamilyEncoderComponentSlot | null,
	_tilt: FamilyEncoderComponentSlot | null,
): PositionJoystickRates {
	return POSITION_DIALOG_DEFAULT_RATES;
}

/** The controlled dialog descriptor of one axis; `value` overrides the model (optimistic draft). */
export function positionDialogAxis(
	model: PositionAxisModel,
	value: number,
	writable: boolean,
): PositionPanValue | PositionTiltValue {
	return {
		value: writable ? value : Number.NaN,
		minimum: model.limits.min,
		maximum: model.limits.max,
		step: model.step,
		keyStep: model.keyStep,
	};
}

/** Identity of what an axis currently reads; a change drops a settled optimistic draft. */
export function positionAxisKey(model: PositionAxisModel) {
	return `${model.mode}:${model.value}`;
}
