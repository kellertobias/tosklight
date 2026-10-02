import type {
	FamilyEncoderComponentSlot,
	ProgrammingColorComponent,
	ProgrammingScalarEdit,
	ProgrammingTargetReference,
} from "../../../../api/familyEncoderModels";
import type { ProgrammerValueTiming } from "../../../../features/programmerValues/contracts";
import {
	createColorGestureSession,
	createFocusGestureSession,
	createZoomGestureSession,
	scalarSet,
	scalarSpread,
	scalarStep,
} from "../../../../features/programmerValues/familyGestureFamilies";
import type {
	FamilyGestureCancelReason,
	FamilyGestureHandle,
	FamilyGestureLane,
	FamilyGestureSession,
	FamilyGestureSessionOptions,
	FamilyGestureStartInput,
} from "../../../../features/programmerValues/familyGestureSession";
import {
	POSITION_TARGET_ORIGIN,
	PositionGestureSession,
	type PositionRepresentation,
} from "../../../../features/programmerValues/positionGestureSession";
import type {
	ColorAdoptionInput,
	ProgrammerValuesHold,
} from "../../../../api/colorAdoptionWire";
import type { NativeColorReferenceRef } from "../../../../api/nativeColorModels";
import { isNativeSlot, nativeOperation } from "./nativeColorSlots";

/**
 * Family encoder binding (TL-549/550/551 UI foundation).
 *
 * Software encoder steps and hardware/OSC `encode/N` detents on a semantic slot both arrive
 * here and become ordered `component_edits` of that slot's component, never a normalized value.
 * One `FamilyGestureSession` per backend owner (position, color, focus, zoom) runs in idle-end
 * encoder mode: consecutive detents on the same owner and target share one gesture (one Undo
 * group) and the gesture ends with exactly one Finish after {@link FAMILY_ENCODER_IDLE_END_MILLIS}
 * without a change. A changed lane, group or fixture set supersedes the open gesture.
 */

export const FAMILY_ENCODER_IDLE_END_MILLIS = 250;

export type FamilyEncoderDetent = "up" | "down" | "left" | "right";

/** What one edit targets. The caller derives it from its projection; nothing is inferred here. */
export interface FamilyEncoderTarget {
	lane: FamilyGestureLane;
	groupId: string | null;
	timing: ProgrammerValueTiming;
	/** Position only: the selection's current representation (unknown counts as not Target). */
	positionRepresentation?: PositionRepresentation;
	/** Position only: the reference a first X/Y/Z edit activates (default Origin). */
	positionTargetReference?: ProgrammingTargetReference;
	/**
	 * Position only (TL-637): the slot's fixtures have no Position physical data and nothing to
	 * adopt, so no Position edit is sent (see `positionSlotUnsupported`).
	 */
	positionUnsupported?: boolean;
}

export interface FamilyEncoderBindingOptions extends FamilyGestureSessionOptions {
	idleEndMillis?: number;
	/** The Point slot's ordered choices; Origin only until a Point source exists (TL-549). */
	pointChoices?(): readonly ProgrammingTargetReference[];
	/** TL-554: the reference head every Direct (native) edit names; none refuses quietly. */
	nativeReference?(): NativeColorReferenceRef | null;
	/** TL-554: the explicit semantic starting colour a new semantic Color gesture names. */
	semanticColorAdoption?(): ColorAdoptionInput | null;
	/** TL-554: holds and outcomes of Color edits (the explicit-start notice). */
	onColorHold?(reason: ProgrammerValuesHold): void;
	onColorOutcome?(outcome: unknown): void;
}

type Owner = "position" | "color" | "focus" | "zoom";

/** Hardware parity: up/down move one descriptor step, left/right ten (the coarse turn). */
export function familyDetentDelta(
	slot: FamilyEncoderComponentSlot,
	value: string | undefined,
) {
	const step = slot.descriptor.step;
	if (value === "up") return step;
	if (value === "down") return -step;
	if (value === "right") return step * 10;
	if (value === "left") return -step * 10;
	return null;
}

/**
 * TL-551: a Zoom slot without a published beam/field convention is never edited. The Focus/Zoom
 * dialog keeps such a request local; the encoders send nothing either and show a quiet
 * unsupported state instead (no error, no toast). The detent is still consumed so the legacy
 * normalized Zoom step never fires in its place.
 */
export function familySlotUnsupported(slot: FamilyEncoderComponentSlot) {
	return slot.component.kind === "zoom" && slot.convention == null;
}

/** A scalar slot whose published descriptor allows an ordered `[THRU]` spread. */
export function familySlotSpreads(slot: FamilyEncoderComponentSlot) {
	return slot.edit === "scalar" && slot.descriptor.spread === true;
}

/** TL-637: a Position edit the caller marked unsupported is never sent (quiet, consumed). */
function positionEditUnsupported(slot: FamilyEncoderComponentSlot, target: FamilyEncoderTarget) {
	return target.positionUnsupported === true && slot.descriptor.owner === "position";
}

function sameReference(
	left: ProgrammingTargetReference | undefined,
	right: ProgrammingTargetReference,
) {
	if (!left || left.kind !== right.kind) return false;
	return left.kind === "origin" || (right.kind === "point" && left.point_id === right.point_id);
}

function colorComponent(slot: FamilyEncoderComponentSlot) {
	return slot.component.kind === "color"
		? (slot.component.component as ProgrammingColorComponent)
		: null;
}

export class FamilyEncoderBinding {
	private readonly position: PositionGestureSession;
	private readonly sessions: Record<
		Exclude<Owner, "position">,
		FamilyGestureSession<unknown>
	>;
	private readonly keys = new Map<Owner, string>();
	private readonly idleEndMillis: number;

	constructor(private readonly options: FamilyEncoderBindingOptions) {
		this.idleEndMillis = options.idleEndMillis ?? FAMILY_ENCODER_IDLE_END_MILLIS;
		this.position = new PositionGestureSession(options);
		this.sessions = {
			color: createColorGestureSession({
				...options,
				onHold: (lane, reason) => {
					options.onHold?.(lane, reason);
					options.onColorHold?.(reason);
				},
				onOutcome: (lane, outcome) => {
					options.onOutcome?.(lane, outcome);
					options.onColorOutcome?.(outcome);
				},
			}) as FamilyGestureSession<unknown>,
			focus: createFocusGestureSession(options) as FamilyGestureSession<unknown>,
			zoom: createZoomGestureSession(options) as FamilyGestureSession<unknown>,
		};
	}

	/** A hardware/OSC detent. Returns whether the slot consumed it (always, for a semantic slot). */
	detent(
		slot: FamilyEncoderComponentSlot,
		value: string | undefined,
		target: FamilyEncoderTarget,
	) {
		const delta = familyDetentDelta(slot, value);
		if (delta === null) return true;
		if (slot.edit === "target_reference")
			this.cycleTarget(slot, delta > 0 ? 1 : -1, target);
		else this.step(slot, delta, target);
		return true;
	}

	/** A relative step in descriptor units (software encoder turn or hardware detent). */
	step(slot: FamilyEncoderComponentSlot, delta: number, target: FamilyEncoderTarget) {
		if (slot.edit !== "scalar" || !Number.isFinite(delta) || delta === 0) return null;
		return this.submit(slot, scalarStep(delta), target, false);
	}

	/** An absolute value in descriptor units (typed entry): one complete gesture. */
	set(slot: FamilyEncoderComponentSlot, value: number, target: FamilyEncoderTarget) {
		if (slot.edit !== "scalar" || !Number.isFinite(value)) return null;
		return this.submit(slot, scalarSet(value), target, true);
	}

	/**
	 * An ordered `[THRU]` spread in descriptor units (typed `A THRU B …` on the software value pad,
	 * the hardware encoder modal or its OSC-driven keypad): one complete gesture that sets this
	 * component per fixture in selection order (per member in Group order). Only a slot whose
	 * descriptor publishes `spread` takes one; at least two finite points are required.
	 */
	spread(
		slot: FamilyEncoderComponentSlot,
		points: readonly number[],
		target: FamilyEncoderTarget,
	) {
		if (!familySlotSpreads(slot) || points.length < 2 || !points.every(Number.isFinite))
			return null;
		return this.submit(slot, scalarSpread(points), target, true);
	}

	/** Activates or re-points Position Target (Point slot): one complete gesture. */
	chooseTarget(
		slot: FamilyEncoderComponentSlot,
		reference: ProgrammingTargetReference,
		target: FamilyEncoderTarget,
	) {
		if (slot.edit !== "target_reference" || positionEditUnsupported(slot, target)) return null;
		const handle = this.gesture("position", slot, target);
		const sent = handle?.change({ target: reference }) ?? null;
		handle?.commit();
		return sent;
	}

	cancel(reason: FamilyGestureCancelReason) {
		this.position.cancel(reason);
		for (const session of Object.values(this.sessions)) session.cancel(reason);
	}

	dispose() {
		this.position.dispose();
		for (const session of Object.values(this.sessions)) session.dispose();
		this.keys.clear();
	}

	private cycleTarget(
		slot: FamilyEncoderComponentSlot,
		direction: 1 | -1,
		target: FamilyEncoderTarget,
	) {
		const choices = this.options.pointChoices?.() ?? [POSITION_TARGET_ORIGIN];
		if (!choices.length) return null;
		const current = choices.findIndex((choice) =>
			sameReference(target.positionTargetReference, choice),
		);
		const next =
			choices[(current + direction + choices.length) % choices.length] ?? null;
		if (!next || (current >= 0 && choices.length === 1)) return null;
		return this.chooseTarget(slot, next, target);
	}

	private submit(
		slot: FamilyEncoderComponentSlot,
		operation: ProgrammingScalarEdit,
		target: FamilyEncoderTarget,
		complete: boolean,
	) {
		if (familySlotUnsupported(slot) || positionEditUnsupported(slot, target)) return null;
		const owner = slot.descriptor.owner as Owner;
		const change = this.change(slot, operation, target);
		if (!change) return null;
		const handle = this.gesture(owner, slot, target, complete);
		const sent = handle?.change(change as never) ?? null;
		// A typed value is a complete request: kept even behind a settling edit.
		if (complete) handle?.commit();
		return sent;
	}

	private change(
		slot: FamilyEncoderComponentSlot,
		operation: ProgrammingScalarEdit,
		target: FamilyEncoderTarget,
	): object | null {
		const component = slot.component.kind;
		if (component === "pan") return { pan: operation };
		if (component === "tilt") return { tilt: operation };
		if (component === "target_x" || component === "target_y" || component === "target_z") {
			const axis = component.slice("target_".length) as "x" | "y" | "z";
			return {
				offset: { [axis]: operation },
				...(target.positionRepresentation === "target"
					? {}
					: { target: target.positionTargetReference ?? POSITION_TARGET_ORIGIN }),
			};
		}
		if (component === "focus") return { focus: operation };
		if (component === "zoom") return { zoom: operation };
		if (slot.component.kind === "native_color") {
			const native = nativeOperation(slot, operation);
			return native
				? { native: [{ binding: slot.component.component, operation: native }] }
				: null;
		}
		const color = colorComponent(slot);
		return color ? { components: [{ component: color, operation }] } : null;
	}

	/** The open gesture of `owner` for this target, or a fresh one (superseding any other). */
	private gesture(
		owner: Owner,
		slot: FamilyEncoderComponentSlot,
		target: FamilyEncoderTarget,
		complete = false,
	): FamilyGestureHandle<unknown> | null {
		const session: FamilyGestureSession<unknown, FamilyGestureStartInput> =
			owner === "position"
				? (this.position as unknown as FamilyGestureSession<unknown>)
				: this.sessions[owner];
		// TL-554: a Direct edit names its reference head; Semantic and Direct never share a
		// gesture, and a changed reference starts a new one.
		const reference = isNativeSlot(slot) ? (this.options.nativeReference?.() ?? null) : null;
		if (isNativeSlot(slot) && !reference) return null;
		const semanticAdoption =
			owner === "color" && !reference
				? (this.options.semanticColorAdoption?.() ?? null)
				: null;
		const key = [
			target.lane,
			target.groupId ?? "",
			slot.fixture_ids.join(","),
			target.positionRepresentation ?? "",
			reference ? `native:${reference.fixture_id}:${reference.head_id}` : "",
			semanticAdoption ? JSON.stringify(semanticAdoption) : "",
		].join("|");
		const active = session.active;
		if (!complete && active?.isOpen && this.keys.get(owner) === key) return active;
		this.keys.set(owner, key);
		return session.start({
			lane: target.lane,
			fixtureIds: target.groupId ? [] : slot.fixture_ids,
			groupId: target.groupId,
			timing: target.timing,
			displayedFixtureIds: slot.fixture_ids,
			...(complete ? {} : { idleEndMillis: this.idleEndMillis }),
			...(owner === "position" && target.positionRepresentation
				? { representation: target.positionRepresentation }
				: {}),
			...(reference
				? {
						colorAdoption: {
							nativeReference: {
								fixtureId: reference.fixture_id,
								headId: reference.head_id,
							},
						},
					}
				: semanticAdoption
					? { colorAdoption: semanticAdoption }
					: {}),
		} as FamilyGestureStartInput);
	}
}
