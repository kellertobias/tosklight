import { useMemo, useRef } from "react";
import { useProgrammerFadeMillis } from "../../../../features/configuration/ConfigurationState";
import {
	useFamilyEncoderPages,
	useFamilyEncodersContext,
} from "../../../../features/familyEncoders/FamilyEncodersProvider";
import { useFamilyReadouts } from "../../../../features/familyEncoders/useFamilyReadouts";
import { capturesProgrammerWrites } from "../../../../features/programmerCaptureMode/contracts";
import { useProgrammerCaptureModeView } from "../../../../features/programmerCaptureMode/ProgrammerCaptureModeView";
import { useProgrammerPreloadValuesActions } from "../../../../features/programmerPreloadValues/ProgrammerPreloadValuesView";
import { useProgrammerValuesActions } from "../../../../features/programmerValues/ProgrammerValuesView";
import { selectedGroupId } from "../../../../features/programmingInteraction/contracts";
import { useProgrammingSelectionView } from "../../../../features/programmingInteraction/ProgrammingInteractionView";
import {
	positionSelectionState,
	type ProgrammerValueEntry,
} from "../../../control/parameterControls/familyEncoders/familyEncoderDisplay";
import { familyGestureWriter } from "../../../control/parameterControls/familyEncoders/useFamilyEncoderBinding";
import {
	immediateParameterTiming,
	type ParameterValuesMutationPort,
	parameterValueTiming,
} from "../../../control/parameterControls/parameterValueMutations";
import { useParameterPreloadValues } from "../../../control/parameterControls/useParameterPreloadValues";
import { useParameterProgrammerValues } from "../../../control/parameterControls/useParameterProgrammerValues";
import type { PositionDialogProps } from "../intention/PositionDialog";
import {
	positionAxisKey,
	positionAxisModel,
	positionAxisSlot,
	positionDialogAxis,
	positionFamilyGroup,
	positionDialogUnsupported,
	positionJoystickRates,
	positionValueCaption,
} from "./positionDialogModel";
import {
	positionDraftValue,
	usePositionDialogGestures,
} from "./usePositionDialogGestures";

/**
 * Production data for the semantic Position Special Dialog (TL-549): the published Position
 * slots (`family-encoder-pages`), the current lane's programmer values, the lane's displayed-source
 * readouts and the lane writers. Returns the controlled `PositionDialog` props minus `onClose`.
 */

const EMPTY: readonly string[] = [];
const EMPTY_VALUES: readonly ProgrammerValueEntry[] = [];

function useLaneValues(fixtureIds: readonly string[], groupId: string | null) {
	const captureMode = useProgrammerCaptureModeView(true);
	const preload = capturesProgrammerWrites(captureMode);
	const normalView = useParameterProgrammerValues(
		fixtureIds,
		groupId,
		captureMode !== null && !preload,
	);
	const preloadView = useParameterPreloadValues(
		fixtureIds,
		groupId,
		captureMode !== null && preload,
	);
	const view = captureMode ? (preload ? preloadView : normalView) : null;
	return {
		lane: captureMode && view?.ready ? (preload ? "preload" : "normal") : null,
		values: (view?.fixtureValues ?? EMPTY_VALUES) as readonly ProgrammerValueEntry[],
	} as const;
}

function useLaneWriters() {
	const normal = useProgrammerValuesActions();
	const preload = useProgrammerPreloadValuesActions();
	const latest = useRef({ normal, preload });
	latest.current = { normal, preload };
	return {
		writerFor: (lane: "normal" | "preload") =>
			familyGestureWriter(
				(lane === "preload"
					? latest.current.preload
					: latest.current.normal) as ParameterValuesMutationPort | null,
			),
	};
}

export function usePositionSpecialDialog(
	selectedFixtureIds: readonly string[],
): Omit<PositionDialogProps, "onClose"> {
	const selection = useProgrammingSelectionView(true);
	const groupId = selectedGroupId(selection);
	const { lane, values } = useLaneValues(selectedFixtureIds, groupId);
	const group = positionFamilyGroup(useFamilyEncoderPages(selectedFixtureIds, true));
	const panSlot = positionAxisSlot(group, "pan");
	const tiltSlot = positionAxisSlot(group, "tilt");
	const fixtureIds = panSlot?.fixture_ids ?? tiltSlot?.fixture_ids ?? EMPTY;
	const readouts = useFamilyReadouts(lane ?? "normal", fixtureIds, {
		enabled: lane !== null,
		consumerId: "position-special-dialog",
	});
	const context = useFamilyEncodersContext();
	const fadeMillis = useProgrammerFadeMillis() ?? undefined;
	const writers = useLaneWriters();
	const { representation } = useMemo(
		() => positionSelectionState(values, fixtureIds),
		[values, fixtureIds],
	);
	const input = { programmerValues: values, readouts: readouts.snapshot, representation };
	const pan = positionAxisModel(panSlot, "pan", input);
	const tilt = positionAxisModel(tiltSlot, "tilt", input);
	const keys = { pan: positionAxisKey(pan), tilt: positionAxisKey(tilt) };
	// TL-637: without Position physical data nothing is sent; the caption says Unsupported.
	const writable =
		lane !== null &&
		fixtureIds.length > 0 &&
		!positionDialogUnsupported(pan, tilt) &&
		writers.writerFor(lane) !== null;
	const { callbacks, draft, returnHome } = usePositionDialogGestures(
		{
			lane: writable ? lane : null,
			fixtureIds,
			groupId,
			timing: lane === "preload" ? parameterValueTiming(fadeMillis) : immediateParameterTiming(),
			representation,
			modes: { pan: pan.mode, tilt: tilt.mode },
			keys,
		},
		{
			writerFor: writers.writerFor,
			// The covering lease of the fixtures this dialog's readouts showed.
			displayedSource: (edited) =>
				context?.readouts.displayedSource(edited, fixtureIds) ?? null,
			onDisplayedSourceHold: () => readouts.reread(),
			onError: (error) => console.warn("Position Special Dialog edit refused", error),
		},
	);
	return {
		pan: positionDialogAxis(pan, positionDraftValue(draft, "pan", keys.pan, pan.value), writable),
		tilt: positionDialogAxis(tilt, positionDraftValue(draft, "tilt", keys.tilt, tilt.value), writable),
		joystick: positionJoystickRates(panSlot, tiltSlot),
		valueCaption: positionValueCaption(pan, tilt),
		// Return Home follows Programmer Fade on every lane, as it always has (docs/help 06).
		returnHome: {
			disabled: !writable,
			onPress: () => void returnHome(parameterValueTiming(fadeMillis)),
		},
		...callbacks,
	};
}
