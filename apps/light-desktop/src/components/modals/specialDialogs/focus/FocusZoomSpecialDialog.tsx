import { useEffect } from "react";
import type { ProgrammerValueEntry } from "../../../control/parameterControls/familyEncoders/familyEncoderDisplay";
import { FocusZoomDialog } from "../intention/FocusZoomDialog";
import {
	focusDialogValue,
	requestedControlValue,
	zoomDialogValue,
} from "./focusZoomDialogModel";
import {
	type FocusZoomGestureEnvironment,
	useFocusZoomGestures,
} from "./useFocusZoomGestures";

export interface FocusZoomSpecialDialogProps {
	environment: FocusZoomGestureEnvironment & {
		/** The capture lane's programmer values for the selection (requested values). */
		programmerValues: readonly ProgrammerValueEntry[];
	};
	close(): void;
}

/**
 * The production Focus Special Dialog body (TL-551): the controlled `FocusZoomDialog` in its
 * standard modal, fed from the published descriptors and the requested programmer values, and
 * writing through one Focus and one Zoom gesture session. Opening, resizing and closing send
 * nothing.
 */
export function FocusZoomSpecialDialog({
	environment,
	close,
}: FocusZoomSpecialDialogProps) {
	const gestures = useFocusZoomGestures(environment);
	const requestedZoom = requestedControlValue(
		environment.zoomSlot,
		environment.programmerValues,
	);
	const requestedFocus = requestedControlValue(
		environment.focusSlot,
		environment.programmerValues,
	);
	const { settle } = gestures;
	// A new store value supersedes the dialog's local request once no drag is open.
	// biome-ignore lint/correctness/useExhaustiveDependencies: keyed on the reflected value
	useEffect(() => settle("zoom"), [requestedZoom.value, requestedZoom.text]);
	// biome-ignore lint/correctness/useExhaustiveDependencies: keyed on the reflected value
	useEffect(() => settle("focus"), [requestedFocus.value, requestedFocus.text]);
	const zoom = zoomDialogValue({
		slot: environment.zoomSlot,
		requested: requestedZoom,
		local: gestures.local.zoom,
		refused: gestures.zoomRefused,
	});
	const focus = focusDialogValue({
		slot: environment.focusSlot,
		requested: requestedFocus,
		local: gestures.local.focus,
	});
	return (
		<FocusZoomDialog
			zoom={zoom.zoom}
			focus={focus.focus}
			zoomStatus={zoom.status}
			focusStatus={focus.status}
			// Until the lane and its requested values are known nothing can be sent: say so.
			disabled={!environment.lane || !environment.ready}
			onZoomChange={gestures.onZoomChange}
			onFocusChange={gestures.onFocusChange}
			onGestureStart={gestures.onGestureStart}
			onGestureEnd={gestures.onGestureEnd}
			onGestureCancel={gestures.onGestureCancel}
			onClose={close}
		/>
	);
}
