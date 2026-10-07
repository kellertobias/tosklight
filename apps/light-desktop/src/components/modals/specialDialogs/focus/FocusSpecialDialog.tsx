import { useMemo } from "react";
import { useProgrammerFadeMillis } from "../../../../features/configuration/ConfigurationState";
import {
	useFamilyEncoderPages,
	useFamilyEncodersContext,
} from "../../../../features/familyEncoders/FamilyEncodersProvider";
import { useFamilyReadouts } from "../../../../features/familyEncoders/useFamilyReadouts";
import { capturesProgrammerWrites } from "../../../../features/programmerCaptureMode/contracts";
import { useProgrammerCaptureModeView } from "../../../../features/programmerCaptureMode/ProgrammerCaptureModeView";
import { useProgrammerPreloadValuesActions } from "../../../../features/programmerPreloadValues/ProgrammerPreloadValuesView";
import type { FamilyGestureLane } from "../../../../features/programmerValues/familyGestureSession";
import { useProgrammerValuesActions } from "../../../../features/programmerValues/ProgrammerValuesView";
import { selectedGroupId } from "../../../../features/programmingInteraction/contracts";
import { useProgrammingSelectionView } from "../../../../features/programmingInteraction/ProgrammingInteractionView";
import type { ProgrammerValueEntry } from "../../../control/parameterControls/familyEncoders/familyEncoderDisplay";
import { familyGestureWriter } from "../../../control/parameterControls/familyEncoders/useFamilyEncoderBinding";
import {
	immediateParameterTiming,
	type ParameterValuesMutationPort,
	parameterValueTiming,
} from "../../../control/parameterControls/parameterValueMutations";
import { useParameterPreloadValues } from "../../../control/parameterControls/useParameterPreloadValues";
import { useParameterProgrammerValues } from "../../../control/parameterControls/useParameterProgrammerValues";
import type { SemanticSpecialDialogProps } from "../registry/specialDialogRegistry";
import { focusZoomSlots } from "./focusZoomDialogModel";
import {
	FocusZoomSpecialDialog,
	type FocusZoomSpecialDialogProps,
} from "./FocusZoomSpecialDialog";

/**
 * The semantic Focus Special Dialog registered for the Focus family (TL-551). It reads the
 * desk's capture lane, the selection's requested values on that lane, the Focus family's published
 * descriptors and the shared displayed-source readouts, then renders {@link FocusZoomSpecialDialog}.
 * Only reachable under the semantic programming contract; contract 0 keeps no Focus dialog.
 */
export function FocusSpecialDialog({
	selectedFixtureIds,
	close,
}: SemanticSpecialDialogProps) {
	const environment = useFocusDialogEnvironment(selectedFixtureIds);
	return <FocusZoomSpecialDialog environment={environment} close={close} />;
}

const EMPTY: readonly ProgrammerValueEntry[] = [];

export function useFocusDialogEnvironment(
	selectedFixtureIds: readonly string[],
): FocusZoomSpecialDialogProps["environment"] {
	const selection = useProgrammingSelectionView(true);
	const groupId = selectedGroupId(selection);
	const slots = focusZoomSlots(useFamilyEncoderPages(selectedFixtureIds, true));
	const captureMode = useProgrammerCaptureModeView(true);
	const preload = capturesProgrammerWrites(captureMode);
	const lane: FamilyGestureLane | null = captureMode
		? preload
			? "preload"
			: "normal"
		: null;
	const normalView = useParameterProgrammerValues(
		selectedFixtureIds,
		groupId,
		lane === "normal",
	);
	const preloadView = useParameterPreloadValues(
		selectedFixtureIds,
		groupId,
		lane === "preload",
	);
	const view = lane === "preload" ? preloadView : lane ? normalView : null;
	const fadeMillis = useProgrammerFadeMillis();
	const normal = useProgrammerValuesActions();
	const preloadActions = useProgrammerPreloadValuesActions();
	const context = useFamilyEncodersContext();
	const ownerFixtures = useMemo(
		() => [
			...new Set([
				...(slots.focus?.fixture_ids ?? []),
				...(slots.zoom?.fixture_ids ?? []),
			]),
		],
		[slots.focus, slots.zoom],
	);
	// Keeps the lane's displayed-source lease fresh while the dialog is open.
	const readouts = useFamilyReadouts(lane ?? "normal", ownerFixtures, {
		enabled: lane !== null,
		consumerId: "focus-special-dialog",
	});
	return {
		focusSlot: slots.focus,
		zoomSlot: slots.zoom,
		lane,
		ready: view?.ready ?? false,
		groupId,
		timing:
			lane === "preload"
				? parameterValueTiming(fadeMillis ?? undefined)
				: immediateParameterTiming(),
		programmerValues: (view?.fixtureValues as readonly ProgrammerValueEntry[]) ?? EMPTY,
		writerFor: (target) =>
			familyGestureWriter(
				(target === "preload" ? preloadActions : normal) as
					| ParameterValuesMutationPort
					| null,
			),
		// The covering lease of the Focus/Zoom owners this dialog's readouts showed.
		displayedSource: (target) =>
			context?.readouts.displayedSource(target, ownerFixtures) ?? null,
		onDisplayedSourceHold: () => readouts.reread(),
	};
}
