import { useEffect, useRef, useState } from "react";
import type { FamilyEncoderComponentSlot } from "../../../../api/familyEncoderModels";
import type { ProgrammerValueTiming } from "../../../../features/programmerValues/contracts";
import type { DisplayedSource } from "../../../../features/programmerValues/displayedSource";
import {
	createFocusGestureSession,
	createZoomGestureSession,
	type FocusGestureChange,
	scalarSet,
	type ZoomGestureChange,
} from "../../../../features/programmerValues/familyGestureFamilies";
import {
	attachGestureWindowGuards,
	type FamilyGestureHandle,
	type FamilyGestureLane,
	type FamilyGestureSession,
	type FamilyGestureWriter,
} from "../../../../features/programmerValues/familyGestureSession";
import type {
	FocusZoomCancelReason,
	FocusZoomControl,
	FocusZoomGesture,
} from "../intention/FocusZoomDialog";

/**
 * Binds the controlled `FocusZoomDialog` to two family gesture sessions (TL-551).
 *
 * - Focus and Zoom are separate backend owners: each has its own session, so a Focus drag and a
 *   Zoom drag never share an Undo group and each Finish names its own attribute.
 * - A dialog gesture is one session gesture on the lane it started on (the writer stays pinned),
 *   naming the displayed-source lease read at start. Pointer moves send scalar `set` component
 *   edits in descriptor units (Zoom degrees, Focus 0–1); release ends, cancel cancels.
 * - Window blur and the document becoming hidden end the open gesture through
 *   `attachGestureWindowGuards`; later moves of that pointer are ignored until it is released.
 * - A keyboard step is one complete gesture that commits: pressed while the previous step is
 *   still settling, it queues behind it in order instead of being dropped.
 * - Zoom without a published convention is never sent: the requested value is kept locally and
 *   shown with a quiet unsupported state. A refused or held Zoom edit does the same.
 */

export interface FocusZoomGestureEnvironment {
	focusSlot: FamilyEncoderComponentSlot | null;
	zoomSlot: FamilyEncoderComponentSlot | null;
	/** The capture lane; `null` until the capture mode is known (nothing is sent). */
	lane: FamilyGestureLane | null;
	ready: boolean;
	groupId: string | null;
	timing: ProgrammerValueTiming;
	writerFor(lane: FamilyGestureLane): FamilyGestureWriter | null;
	displayedSource?(lane: FamilyGestureLane): DisplayedSource | null;
	onDisplayedSourceHold?(lane: FamilyGestureLane): void;
	onError?(error: Error): void;
}

type Sessions = {
	focus: FamilyGestureSession<FocusGestureChange>;
	zoom: FamilyGestureSession<ZoomGestureChange>;
};

export interface FocusZoomGestureState {
	/** The latest requested value per control, until the server reflects (or refuses) it. */
	local: Partial<Record<FocusZoomControl, number>>;
	zoomRefused: boolean;
	active: Partial<Record<FocusZoomControl, boolean>>;
	onGestureStart(gesture: FocusZoomGesture): void;
	onZoomChange(value: number, gesture: FocusZoomGesture): void;
	onFocusChange(value: number, gesture: FocusZoomGesture): void;
	onGestureEnd(gesture: FocusZoomGesture): void;
	onGestureCancel(gesture: FocusZoomGesture, reason: FocusZoomCancelReason): void;
	/** The store reflected a new requested value: drop the local one unless a drag is open. */
	settle(control: FocusZoomControl): void;
}

function useSessions(environment: FocusZoomGestureEnvironment) {
	const latest = useRef(environment);
	latest.current = environment;
	const [sessions, setSessions] = useState<Sessions | null>(null);
	useEffect(() => {
		const options = {
			writerFor: (lane: FamilyGestureLane) => latest.current.writerFor(lane),
			displayedSource: (lane: FamilyGestureLane) =>
				latest.current.displayedSource?.(lane) ?? null,
			onDisplayedSourceHold: (lane: FamilyGestureLane) =>
				latest.current.onDisplayedSourceHold?.(lane),
			onError: (error: Error) => latest.current.onError?.(error),
		};
		const next: Sessions = {
			focus: createFocusGestureSession(options),
			zoom: createZoomGestureSession(options),
		};
		const detach = [
			attachGestureWindowGuards(next.focus),
			attachGestureWindowGuards(next.zoom),
		];
		setSessions(next);
		return () => {
			for (const stop of detach) stop();
			next.focus.dispose();
			next.zoom.dispose();
		};
	}, []);
	return { sessions, latest };
}

/** An edit that left the programmer unchanged because the server refused or held it. */
function refusedOutcome(outcome: unknown) {
	return (
		outcome === null ||
		(typeof outcome === "object" &&
			outcome !== null &&
			"hold" in outcome &&
			(outcome as { hold?: unknown }).hold !== undefined)
	);
}

export function useFocusZoomGestures(
	environment: FocusZoomGestureEnvironment,
): FocusZoomGestureState {
	const { sessions, latest } = useSessions(environment);
	const handles = useRef(new Map<number, FamilyGestureHandle<unknown> | null>());
	const [local, setLocal] = useState<Partial<Record<FocusZoomControl, number>>>({});
	const [active, setActive] = useState<Partial<Record<FocusZoomControl, boolean>>>({});
	const [zoomRefused, setZoomRefused] = useState(false);
	// Sent edits not yet answered, per control. While one is outstanding, a reflected value is an
	// older step's: the dialog keeps its latest request, so the next key steps from it.
	const outstanding = useRef<Record<FocusZoomControl, number>>({ zoom: 0, focus: 0 });
	const activeNow = useRef<Partial<Record<FocusZoomControl, boolean>>>({});
	const dropLocal = (control: FocusZoomControl) =>
		setLocal((current) =>
			current[control] === undefined ? current : { ...current, [control]: undefined },
		);

	const start = (gesture: FocusZoomGesture) => {
		const env = latest.current;
		const slot = gesture.control === "zoom" ? env.zoomSlot : env.focusSlot;
		const sendable =
			slot &&
			env.lane &&
			env.ready &&
			(gesture.control === "focus" || slot.convention != null);
		const session = sessions?.[gesture.control];
		const handle =
			sendable && session && env.lane
				? (session as FamilyGestureSession<unknown>).start({
						lane: env.lane,
						fixtureIds: env.groupId ? [] : slot.fixture_ids,
						groupId: env.groupId,
						timing: env.timing,
					})
				: null;
		// A Zoom the server cannot take is still kept as the operator's requested value.
		const localOnly = gesture.control === "zoom" && Boolean(slot) && !handle;
		if (!handle && !localOnly) return;
		handles.current.set(gesture.id, handle);
		activeNow.current[gesture.control] = true;
		setActive((current) => ({ ...current, [gesture.control]: true }));
	};

	const change = (value: number, gesture: FocusZoomGesture) => {
		if (!handles.current.has(gesture.id)) return;
		const handle = handles.current.get(gesture.id) ?? null;
		if (handle && !handle.isOpen) return; // ended by blur/hidden: ignore the rest of the drag
		setLocal((current) => ({ ...current, [gesture.control]: value }));
		if (!handle) {
			if (gesture.control === "zoom") setZoomRefused(true);
			return;
		}
		const edit =
			gesture.control === "zoom" ? { zoom: scalarSet(value) } : { focus: scalarSet(value) };
		const sent = handle.change(edit);
		if (!sent) return;
		const control = gesture.control;
		outstanding.current[control] += 1;
		void sent.then((outcome) => {
			outstanding.current[control] -= 1;
			const refused = refusedOutcome(outcome);
			if (control === "zoom") setZoomRefused(refused);
			// The lane writer settles the store before it answers: the last answer of a burst
			// hands the display back to the reflected value (a refusal keeps the request shown).
			if (!refused && outstanding.current[control] === 0 && !activeNow.current[control])
				dropLocal(control);
		});
	};

	const finish = (gesture: FocusZoomGesture, reason?: FocusZoomCancelReason) => {
		if (!handles.current.has(gesture.id)) return;
		const handle = handles.current.get(gesture.id) ?? null;
		handles.current.delete(gesture.id);
		if (reason) handle?.cancel(reason);
		// A key step is a complete request: it is kept and sent even behind a settling step.
		else if (gesture.source === "keyboard") handle?.commit();
		else handle?.end();
		activeNow.current[gesture.control] = false;
		setActive((current) => ({ ...current, [gesture.control]: false }));
	};

	return {
		local,
		zoomRefused,
		active,
		onGestureStart: start,
		onZoomChange: change,
		onFocusChange: change,
		onGestureEnd: (gesture) => finish(gesture),
		onGestureCancel: (gesture, reason) => finish(gesture, reason),
		settle: (control) => {
			if (active[control] || outstanding.current[control] > 0) return;
			dropLocal(control);
		},
	};
}
