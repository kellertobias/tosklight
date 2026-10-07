import { immediateParameterTiming } from "../../../../control/parameterControls/parameterValueMutations";
import { useEffect, useRef, useState } from "react";
import { colorAdoptionNotice } from "../../../../../features/familyEncoders/colorAdoptionNotice";
import { useFamilyEncodersContext } from "../../../../../features/familyEncoders/FamilyEncodersProvider";
import {
	type ColorComponentChange,
	type ColorGestureChange,
	createColorGestureSession,
} from "../../../../../features/programmerValues/familyGestureFamilies";
import {
	attachGestureWindowGuards,
	type FamilyGestureCancelReason,
	type FamilyGestureHandle,
	type FamilyGestureSession,
} from "../../../../../features/programmerValues/familyGestureSession";
import { familyGestureWriter } from "../../../../control/parameterControls/familyEncoders/useFamilyEncoderBinding";
import type {
	RangeGesture,
	RangeGestureCallbacks,
	RangeGestureCancelReason,
} from "../HorizontalRangeFader";
import type { ColorDialogLane } from "./useColorDialogLane";

/** The pickers' cancel reasons in the family session's vocabulary. */
export function familyCancelReason(
	reason: RangeGestureCancelReason,
): FamilyGestureCancelReason {
	return reason === "external" ? "superseded" : reason;
}

export interface ColorGestures extends Required<RangeGestureCallbacks> {
	/** Sends one change sample of the gesture that started with `gesture`. */
	change(gesture: RangeGesture, components: readonly ColorComponentChange[]): void;
	/**
	 * Sent samples are not all answered yet: a reflected request meanwhile is an older one, so
	 * the dialog keeps its draft and the next key steps from the operator's latest request.
	 */
	settling: boolean;
}

/**
 * One Color `FamilyGestureSession` for the open dialog (TL-556 Color builder).
 *
 * Every picker or fader gesture becomes one family gesture on the lane it started on, with a
 * fresh Undo group, the selection's displayed-source lease and exactly one Finish. A gesture
 * with no applicable selection never starts: it is a quiet no-op that sends nothing.
 */
export function useColorGestures(
	lane: ColorDialogLane,
	onDisplayedSourceHold: () => void,
): ColorGestures {
	const context = useFamilyEncodersContext();
	const latest = useRef({ lane, context, onDisplayedSourceHold });
	latest.current = { lane, context, onDisplayedSourceHold };
	const [session, setSession] = useState<FamilyGestureSession<ColorGestureChange> | null>(null);
	const handles = useRef(new Map<number, FamilyGestureHandle<ColorGestureChange>>());
	const nextId = useRef(1);
	const outstanding = useRef(0);
	const [settling, setSettling] = useState(false);
	useEffect(() => {
		const created = createColorGestureSession({
			writerFor: (name) =>
				familyGestureWriter(latest.current.lane.writers[name]),
			// The covering lease of the dialog's Color fixtures, never merely the newest.
			displayedSource: (name) =>
				latest.current.context?.readouts.displayedSource(
					name,
					latest.current.lane.colorFixtureIds,
				) ?? null,
			onDisplayedSourceHold: () => latest.current.onDisplayedSourceHold(),
			// TL-554: the explicit-start notice and the reported adoption.
			// Leaving Preload mid-gesture continues the motion on the Normal Programmer.
			currentLane: () => latest.current.lane.lane,
			laneTiming: (name) => (name === "normal" ? immediateParameterTiming() : null),
			onHold: (_lane, reason) => colorAdoptionNotice.held(reason),
			onOutcome: (_lane, outcome) => colorAdoptionNotice.outcome(outcome),
			// Writer and transport failures surface through the lane writer's own error
			// projection; a local refusal is a programming error the tests pin down.
			onError: () => undefined,
		});
		const detach = attachGestureWindowGuards(created);
		const open = handles.current;
		setSession(created);
		return () => {
			detach();
			created.dispose();
			open.clear();
			setSession((current) => (current === created ? null : current));
		};
	}, []);
	const handleOf = (gesture: RangeGesture) =>
		typeof gesture.id === "number" ? handles.current.get(gesture.id) : undefined;
	return {
		onGestureStart() {
			const { lane: target } = latest.current;
			const hasTargets = target.colorFixtureIds.length > 0 || Boolean(target.groupId);
			if (!session || !target.lane || !hasTargets) return undefined;
			const handle = session.start({
				lane: target.lane,
				fixtureIds: target.groupId ? [] : target.colorFixtureIds,
				groupId: target.groupId,
				timing: target.timing,
				colorAdoption: colorAdoptionNotice.semanticInput(),
			});
			if (!handle) return undefined;
			const id = nextId.current++;
			handles.current.set(id, handle);
			return id;
		},
		settling,
		change(gesture, components) {
			if (!components.length) return;
			const sent = handleOf(gesture)?.change({ components });
			if (!sent) return;
			outstanding.current += 1;
			setSettling(true);
			void sent.then(() => {
				outstanding.current -= 1;
				if (outstanding.current === 0) setSettling(false);
			});
		},
		onGestureEnd(gesture) {
			// A keyboard or native step is a complete request, kept even behind a settling one.
			if (gesture.source === "pointer") handleOf(gesture)?.end();
			else handleOf(gesture)?.commit();
			if (typeof gesture.id === "number") handles.current.delete(gesture.id);
		},
		onGestureCancel(gesture, reason) {
			handleOf(gesture)?.cancel(familyCancelReason(reason));
			if (typeof gesture.id === "number") handles.current.delete(gesture.id);
		},
	};
}
