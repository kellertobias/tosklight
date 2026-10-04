import { useEffect, useLayoutEffect, useRef, useState } from "react";
import type { ProgrammerValueTiming } from "../../../../features/programmerValues/contracts";
import type {
	FamilyGestureLane,
	FamilyGestureSessionOptions,
} from "../../../../features/programmerValues/familyGestureSession";
import {
	attachPositionGestureWindowGuards,
	type PositionGestureChange,
	type PositionGestureHandle,
	PositionGestureSession,
	type PositionRepresentation,
	positionAngleSet,
	positionAngleStep,
} from "../../../../features/programmerValues/positionGestureSession";
import type {
	PositionCancelReason,
	PositionChange,
	PositionDialogProps,
	PositionGesture,
} from "../intention/PositionDialog";
import type { PositionAxis, PositionAxisMode } from "./positionDialogModel";

/**
 * Binds the controlled `PositionDialog` to one `PositionGestureSession` (TL-549).
 *
 * - Every dialog gesture (pointer drag, joystick hold, key or button step) is one session
 *   gesture: one fresh Undo group, the lane's writer pinned at start (Normal or Preload by the
 *   current capture mode), the displayed-source lease read once at start, and exactly one Finish
 *   on release, cancel, close, blur, hidden, supersede or unmount.
 * - Absolute axes send `set` angles; a Mixed (relative) axis sends the difference between
 *   consecutive samples, so per-fixture spreads survive.
 * - While Target is active, only the gesture's first sent change states the Target
 *   representation, so `activate_angles` (the one adoption of the displayed pose) is sent once.
 * - Window guards cancel the session's open gesture on blur and on the document becoming hidden,
 *   independently of the dialog's own producer stop.
 * - The draft keeps the dialog on its own emitted values while a gesture is open, so a lagging
 *   readout never pulls a held joystick back. After the gesture it applies only while the axis
 *   still reads what it read at the start (`positionDraftValue`).
 */

export interface PositionDialogEditContext {
	/** The lane edits author into; `null` while the capture mode or values are not ready. */
	lane: FamilyGestureLane | null;
	fixtureIds: readonly string[];
	groupId: string | null;
	timing: ProgrammerValueTiming;
	representation: PositionRepresentation | undefined;
	modes: Record<PositionAxis, PositionAxisMode>;
	/** What each axis reads now (`positionAxisKey`). */
	keys: Record<PositionAxis, string>;
}

export type PositionDialogSessionOptions = Pick<
	FamilyGestureSessionOptions,
	"writerFor" | "displayedSource" | "onDisplayedSourceHold" | "onError" | "createId" | "timers"
>;

export interface PositionDialogDraft {
	open: boolean;
	basis: Record<PositionAxis, string>;
	values: Partial<Record<PositionAxis, number>>;
	/**
	 * Sent steps are not all answered yet. A reflection meanwhile is an older step's, so the
	 * draft stays shown and the next key steps from the operator's latest request.
	 */
	settling?: boolean;
	/** The model moved off `basis` after the draft closed, while it was still settling. */
	overtaken?: boolean;
}

interface OpenGesture {
	id: number;
	handle: PositionGestureHandle;
	representation: PositionRepresentation | undefined;
	modes: Record<PositionAxis, PositionAxisMode>;
	last: Record<PositionAxis, number>;
	sent: boolean;
}

type GestureCallbacks = Pick<
	PositionDialogProps,
	"onChange" | "onGestureStart" | "onGestureEnd" | "onGestureCancel"
>;

const AXES: readonly PositionAxis[] = ["pan", "tilt"];

/** One dialog sample as a Position gesture change, in the axis' mode. */
export function positionDialogChange(
	change: PositionChange,
	gesture: Pick<OpenGesture, "modes" | "last" | "representation" | "sent">,
): PositionGestureChange | null {
	const next: PositionGestureChange = {
		representation: gesture.sent ? "angles" : gesture.representation,
	};
	for (const axis of AXES) {
		const value = change[axis];
		if (value === undefined || !Number.isFinite(value)) continue;
		if (gesture.modes[axis] === "absolute") next[axis] = positionAngleSet(value);
		else if (gesture.modes[axis] === "relative") {
			const delta = value - gesture.last[axis];
			if (delta !== 0) next[axis] = positionAngleStep(delta);
		}
	}
	return next.pan || next.tilt ? next : null;
}

function useSession(options: PositionDialogSessionOptions) {
	const latest = useRef(options);
	useLayoutEffect(() => {
		latest.current = options;
	});
	const session = useRef<PositionGestureSession | null>(null);
	useEffect(() => {
		const next = new PositionGestureSession({
			writerFor: (lane) => latest.current.writerFor(lane),
			displayedSource: (lane, fixtureIds) =>
				latest.current.displayedSource?.(lane, fixtureIds) ?? null,
			onDisplayedSourceHold: (lane) => latest.current.onDisplayedSourceHold?.(lane),
			onError: (error) => latest.current.onError?.(error),
			...(options.createId ? { createId: options.createId } : {}),
			...(options.timers ? { timers: options.timers } : {}),
		});
		session.current = next;
		const detach = attachPositionGestureWindowGuards(next);
		return () => {
			detach();
			next.dispose();
			if (session.current === next) session.current = null;
		};
		// The session lives as long as the dialog; options are read through `latest`.
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, []);
	return session;
}

/** The value the dialog shows for `axis`: the open or still-unsettled draft, else the model. */
export function positionDraftValue(
	draft: PositionDialogDraft | null,
	axis: PositionAxis,
	key: string,
	value: number,
) {
	const drafted = draft?.values[axis];
	if (drafted === undefined) return value;
	return draft?.open || draft?.settling || draft?.basis[axis] === key ? drafted : value;
}

/** The semantic home pose: Pan 0° and Tilt 0°, the centre of travel (the neutral beam). */
export const POSITION_HOME = { pan: 0, tilt: 0 } as const;

/**
 * Return Home as one Position change: absolute home Angles on both axes, whatever the axis mode
 * (a Mixed selection still lands every head at home, never a relative offset). While Target is
 * active the change states it, so the one `activate_angles` takeover precedes the angles.
 */
export function positionReturnHomeChange(
	representation: PositionRepresentation | undefined,
): PositionGestureChange {
	return {
		representation,
		pan: positionAngleSet(POSITION_HOME.pan),
		tilt: positionAngleSet(POSITION_HOME.tilt),
	};
}

export function usePositionDialogGestures(
	context: PositionDialogEditContext,
	options: PositionDialogSessionOptions,
): {
	callbacks: GestureCallbacks;
	draft: PositionDialogDraft | null;
	/**
	 * Return Home: one complete gesture (one Undo step, one Finish) on the current lane and
	 * selection or Group, with `timing` (Programmer Fade). Returns whether a request was sent.
	 */
	returnHome(timing: ProgrammerValueTiming): boolean;
} {
	const session = useSession(options);
	const latest = useRef(context);
	useLayoutEffect(() => {
		latest.current = context;
	});
	const open = useRef<OpenGesture | null>(null);
	const [draft, setDraft] = useState<PositionDialogDraft | null>(null);
	const outstanding = useRef(0);
	const track = (sent: Promise<unknown> | null) => {
		if (!sent) return false;
		outstanding.current += 1;
		setDraft((value) => (value && !value.settling ? { ...value, settling: true } : value));
		void sent.then(() => {
			outstanding.current -= 1;
			if (outstanding.current === 0)
				setDraft((value) => (value?.settling ? { ...value, settling: false } : value));
		});
		return true;
	};

	const finish = (gesture: PositionGesture, reason: "release" | PositionCancelReason) => {
		const current = open.current;
		if (!current || current.id !== gesture.id) return;
		open.current = null;
		// A key or button step is a complete request (kept even behind a settling step); a
		// drag or a held joystick is motion, whose unsent samples stop with the release.
		if (reason !== "release") current.handle.cancel(reason);
		else if (gesture.source !== "pointer" && gesture.control !== "joystick")
			current.handle.commit();
		else current.handle.end();
		setDraft((value) => (value ? { ...value, open: false } : value));
	};
	// A closed draft is overtaken once the model has moved off its basis (an unchanged axis
	// carries the same value in both), even while its requests are still settling. A settled,
	// overtaken draft is dropped, so an Undo back to that basis shows the model, not the step.
	const keyPan = context.keys.pan;
	const keyTilt = context.keys.tilt;
	useEffect(() => {
		if (!draft || draft.open) return;
		const moved = keyPan !== draft.basis.pan || keyTilt !== draft.basis.tilt;
		if (!draft.settling && (moved || draft.overtaken)) setDraft(null);
		else if (draft.settling && moved && !draft.overtaken)
			setDraft((value) => (value ? { ...value, overtaken: true } : value));
	}, [draft, keyPan, keyTilt]);

	const callbacks: GestureCallbacks = {
		onGestureStart: (gesture) => {
			const edit = latest.current;
			setDraft((value) => ({
				open: true,
				basis: { ...edit.keys },
				values: value?.settling ? { ...value.values } : {},
				settling: Boolean(value?.settling),
			}));
			if (!edit.lane || !session.current) return;
			const handle = session.current.start({
				lane: edit.lane,
				fixtureIds: edit.groupId ? [] : edit.fixtureIds,
				groupId: edit.groupId,
				timing: edit.timing,
				representation: edit.representation,
			});
			open.current = handle
				? {
						id: gesture.id,
						handle,
						representation: edit.representation,
						modes: { ...edit.modes },
						last: { pan: gesture.initialPan, tilt: gesture.initialTilt },
						sent: false,
					}
				: null;
		},
		onChange: (change, gesture) => {
			setDraft((value) =>
				value ? { ...value, values: { ...value.values, ...change } } : value,
			);
			const current = open.current;
			if (!current || current.id !== gesture.id) return;
			const next = positionDialogChange(change, current);
			if (change.pan !== undefined) current.last.pan = change.pan;
			if (change.tilt !== undefined) current.last.tilt = change.tilt;
			if (next && track(current.handle.change(next))) current.sent = true;
		},
		onGestureEnd: (gesture) => finish(gesture, "release"),
		onGestureCancel: (gesture, reason) => finish(gesture, reason),
	};
	const returnHome = (timing: ProgrammerValueTiming) => {
		const edit = latest.current;
		if (!edit.lane || !session.current || open.current) return false;
		const handle = session.current.start({
			lane: edit.lane,
			fixtureIds: edit.groupId ? [] : edit.fixtureIds,
			groupId: edit.groupId,
			timing,
			representation: edit.representation,
		});
		if (!handle) return false;
		const sent = track(handle.change(positionReturnHomeChange(edit.representation)));
		// A press is a complete request: kept even behind a settling step, then one Finish.
		handle.commit();
		if (sent)
			setDraft({ open: false, basis: { ...edit.keys }, values: { ...POSITION_HOME }, settling: true });
		return sent;
	};
	return { callbacks, draft, returnHome };
}
