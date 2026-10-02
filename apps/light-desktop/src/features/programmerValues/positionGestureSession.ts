import type {
	ProgrammingComponentEdit,
	ProgrammingScalarEdit,
	ProgrammingTargetReference,
} from "../../api/generated/light-wire";
import {
	attachGestureWindowGuards,
	FamilyGestureEditRefusedError,
	type FamilyGestureFamily,
	type FamilyGestureHandle,
	FamilyGestureSession,
	type FamilyGestureSessionOptions,
	type FamilyGestureStartInput,
} from "./familyGestureSession";

/**
 * Surface-neutral Position gesture session (TL-556): a thin wrapper over the family-neutral
 * `FamilyGestureSession` with the Position attribute and the Position edit builder. The public
 * API is unchanged from the Pan/Tilt-only version; Target and X/Y/Z offset edits are additive.
 *
 * Target contract (accepted, mirrors `crates/shared/core/src/programming/edit.rs`):
 * - Scalar offsets (`target_x/y/z`) never invent a Target. While Position is native-only or
 *   Angles, the first offset edit must carry an explicit `target` reference (Origin or a stable
 *   Point); the builder emits `[target{reference}, target_x?, target_y?, target_z?]` so both
 *   reach the backend in ONE `apply_intent`.
 * - While Target is active, an offset edit without a reference keeps the active reference and
 *   every untouched offset. A reference edit on an active Target replaces only the reference.
 * - Angle (`activate_angles`, Pan, Tilt) and Target edits are exclusive in one change.
 *
 * The session never guesses whether Target is active. The caller states the current
 * representation, per change (`change.representation`) or as the gesture default
 * (`start({ representation })`), from its own projection. Unknown is treated as "not Target":
 * an offset without a reference is then refused (reported, nothing sent). Re-sending the same
 * reference with an offset while the projection has not yet caught up is safe: the backend
 * replaces the reference with itself and keeps the offsets.
 *
 * Centering an aim pad is a change of rate, not an end: the surface keeps the gesture open.
 */

export type {
	FamilyGestureCancelReason as PositionGestureCancelReason,
	FamilyGestureEndReason as PositionGestureEndReason,
	FamilyGestureFinishInput as PositionGestureFinishInput,
	FamilyGestureIntentInput as PositionGestureIntentInput,
	FamilyGestureLane as PositionGestureLane,
	FamilyGestureTimers as PositionGestureTimers,
	FamilyGestureWriter as PositionGestureWriter,
} from "./familyGestureSession";

/** The attribute every Position gesture authors and finishes. */
export const POSITION_GESTURE_ATTRIBUTE = "position";

/** The caller's current Position representation. `native` = no semantic Position value yet. */
export type PositionRepresentation = "native" | "angles" | "target";

export type PositionGestureSessionOptions = FamilyGestureSessionOptions;

export interface PositionGestureStartInput extends FamilyGestureStartInput {
	/** The gesture's default representation; a change's own `representation` wins. */
	representation?: PositionRepresentation;
}

/** Target offset edits in metres; only the present axes are edited. */
export interface PositionTargetOffsetChange {
	x?: ProgrammingScalarEdit;
	y?: ProgrammingScalarEdit;
	z?: ProgrammingScalarEdit;
}

/** One change sample. Only the present components are edited. */
export interface PositionGestureChange {
	pan?: ProgrammingScalarEdit;
	tilt?: ProgrammingScalarEdit;
	/** Prefix an explicit Angle activation (Target to Angle takeover). */
	activateAngles?: boolean;
	/** Explicit Target reference: Origin or a stable Point. Activates or re-points Target. */
	target?: ProgrammingTargetReference;
	/** Target offsets in metres. Refused without `target` unless Target is stated active. */
	offset?: PositionTargetOffsetChange;
	/** Caller-supplied current representation; overrides the start default. */
	representation?: PositionRepresentation;
}

export type PositionGestureHandle = FamilyGestureHandle<PositionGestureChange>;

/** An absolute angle in degrees (unwrapped Pan is a valid value). */
export function positionAngleSet(degrees: number): ProgrammingScalarEdit {
	return { kind: "set", value: { kind: "value", value: degrees } };
}

/** A relative angle step in degrees, for encoders and client-integrated rate motion. */
export function positionAngleStep(degrees: number): ProgrammingScalarEdit {
	return { kind: "relative", value: degrees };
}

/** An absolute Target offset in metres. */
export function positionOffsetSet(metres: number): ProgrammingScalarEdit {
	return { kind: "set", value: { kind: "value", value: metres } };
}

/** A relative Target offset step in metres. */
export function positionOffsetStep(metres: number): ProgrammingScalarEdit {
	return { kind: "relative", value: metres };
}

export const POSITION_TARGET_ORIGIN: ProgrammingTargetReference = {
	kind: "origin",
};

/** A Target reference to a stable Point (non-nil UUID; the wire validator rejects others). */
export function positionTargetPoint(
	pointId: string,
): ProgrammingTargetReference {
	return { kind: "point", point_id: pointId };
}

const OFFSET_COMPONENTS = [
	["x", "target_x"],
	["y", "target_y"],
	["z", "target_z"],
] as const;

function angleEdits(
	change: PositionGestureChange,
	representation: PositionRepresentation | undefined,
) {
	const edits: ProgrammingComponentEdit[] = [];
	const axes = change.pan !== undefined || change.tilt !== undefined;
	// An explicit flag, or Pan/Tilt while the caller states Target is active, takes over.
	if (change.activateAngles || (axes && representation === "target"))
		edits.push({ kind: "activate_angles" });
	if (change.pan)
		edits.push({
			kind: "scalar",
			component: { kind: "pan" },
			operation: change.pan,
		});
	if (change.tilt)
		edits.push({
			kind: "scalar",
			component: { kind: "tilt" },
			operation: change.tilt,
		});
	return edits;
}

function targetEdits(
	change: PositionGestureChange,
	representation: PositionRepresentation | undefined,
) {
	const edits: ProgrammingComponentEdit[] = [];
	const offsets: ProgrammingComponentEdit[] = [];
	for (const [axis, kind] of OFFSET_COMPONENTS) {
		const operation = change.offset?.[axis];
		if (operation)
			offsets.push({ kind: "scalar", component: { kind }, operation });
	}
	if (change.target) edits.push({ kind: "target", reference: change.target });
	else if (offsets.length > 0 && representation !== "target")
		throw new FamilyGestureEditRefusedError(
			"a Target offset edit needs an explicit Target reference while Target is not active",
		);
	return [...edits, ...offsets];
}

/**
 * Builds the ordered Position component edits of one change sample:
 * `[activate_angles?, pan?, tilt?]` or `[target?, target_x?, target_y?, target_z?]`.
 * Throws `FamilyGestureEditRefusedError` for a change the Position contract forbids.
 * `representation` is the caller's statement of the current Position representation.
 */
export function positionComponentEdits(
	change: PositionGestureChange,
	representation: PositionRepresentation | undefined = change.representation,
): ProgrammingComponentEdit[] {
	const angles = angleEdits(change, representation);
	const target = targetEdits(change, representation);
	if (angles.length > 0 && target.length > 0)
		throw new FamilyGestureEditRefusedError(
			"Angle and Target edits are exclusive in one Position change",
		);
	return angles.length > 0 ? angles : target;
}

/** The Position family: attribute `position` and the Position edit builder. */
export const POSITION_GESTURE_FAMILY: FamilyGestureFamily<
	PositionGestureChange,
	PositionGestureStartInput
> = {
	attribute: POSITION_GESTURE_ATTRIBUTE,
	buildEdits: (change, start) =>
		positionComponentEdits(
			change,
			change.representation ?? start.representation,
		),
};

export class PositionGestureSession extends FamilyGestureSession<
	PositionGestureChange,
	PositionGestureStartInput
> {
	constructor(options: PositionGestureSessionOptions) {
		super(POSITION_GESTURE_FAMILY, options);
	}
}

/** The family-neutral window guards under their historical Position name. */
export const attachPositionGestureWindowGuards = attachGestureWindowGuards;
