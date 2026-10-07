import { useMemo } from "react";
import { useProgrammerFadeMillis } from "../../../../../features/configuration/ConfigurationState";
import { useFamilyEncoderPages } from "../../../../../features/familyEncoders/FamilyEncodersProvider";
import { useSelectedPatchedFixtures } from "../../../../../features/patch/PatchState";
import { capturesProgrammerWrites } from "../../../../../features/programmerCaptureMode/contracts";
import { useProgrammerCaptureModeView } from "../../../../../features/programmerCaptureMode/ProgrammerCaptureModeView";
import { useProgrammerPreloadValuesActions } from "../../../../../features/programmerPreloadValues/ProgrammerPreloadValuesView";
import { useProgrammerValuesActions } from "../../../../../features/programmerValues/ProgrammerValuesView";
import type { ProgrammerValueTiming } from "../../../../../features/programmerValues/contracts";
import { selectedGroupId } from "../../../../../features/programmingInteraction/contracts";
import { useProgrammingSelectionView } from "../../../../../features/programmingInteraction/ProgrammingInteractionView";
import {
	immediateParameterTiming,
	type ParameterValuesMutationPort,
	parameterValueTiming,
} from "../../../../control/parameterControls/parameterValueMutations";
import { useParameterPreloadValues } from "../../../../control/parameterControls/useParameterPreloadValues";
import { useParameterProgrammerValues } from "../../../../control/parameterControls/useParameterProgrammerValues";
import { selectedFixtureIdsSupportingAttribute } from "../../../specialColor";
import {
	type ColorDescriptors,
	type ColorValueEntry,
	colorDescriptors,
	colorDialogVariant,
} from "./colorDialogModel";

export type ColorDialogLaneName = "normal" | "preload";

/** Everything the semantic Color dialog reads about its selection and Programmer lane. */
export interface ColorDialogLane {
	/** The lane edits go to, `null` while the capture mode is unknown. */
	lane: ColorDialogLaneName | null;
	ready: boolean;
	fixtureIds: readonly string[];
	groupId: string | null;
	timing: ProgrammerValueTiming;
	descriptors: ColorDescriptors;
	/** The selected owners that carry Color, in selection order. */
	colorFixtureIds: readonly string[];
	variant: "lamp" | "media";
	values: readonly ColorValueEntry[];
	writers: {
		normal: ParameterValuesMutationPort | null;
		preload: ParameterValuesMutationPort | null;
	};
}

const MEDIA_COLOR_ATTRIBUTES = ["media.grayscale"];

/**
 * The selection, lane and requested values of the open Color dialog. Edits take the same lane
 * as the encoders: Preload while capture mode captures Programmer writes, Normal otherwise.
 * Normal edits are immediate (an encoder gesture); Preload edits use Programmer Fade.
 */
export function useColorDialogLane(
	open: boolean,
	selectedFixtureIds: readonly string[],
): ColorDialogLane {
	const selection = useProgrammingSelectionView(open);
	const groupId = selectedGroupId(selection);
	const captureMode = useProgrammerCaptureModeView(open);
	const preloadCapture = capturesProgrammerWrites(captureMode);
	const lane: ColorDialogLaneName | null = captureMode
		? preloadCapture
			? "preload"
			: "normal"
		: null;
	const normalValues = useParameterProgrammerValues(
		selectedFixtureIds,
		groupId,
		open && lane === "normal",
	);
	const preloadValues = useParameterPreloadValues(
		selectedFixtureIds,
		groupId,
		open && lane === "preload",
	);
	const view = lane === "preload" ? preloadValues : normalValues;
	const fadeMillis = useProgrammerFadeMillis() ?? undefined;
	const snapshot = useFamilyEncoderPages(selectedFixtureIds, open);
	const fixtures = useSelectedPatchedFixtures(selectedFixtureIds, open);
	const normal = useProgrammerValuesActions() as ParameterValuesMutationPort | null;
	const preload =
		useProgrammerPreloadValuesActions() as ParameterValuesMutationPort | null;
	const descriptors = useMemo(() => colorDescriptors(snapshot), [snapshot]);
	const colorGroupIds = snapshot?.families.find(
		(group) => group.family === "color",
	)?.fixture_ids;
	const colorFixtureIds = useMemo(() => {
		if (!colorGroupIds) return selectedFixtureIds;
		const color = new Set(colorGroupIds);
		return selectedFixtureIds.filter((id) => color.has(id));
	}, [colorGroupIds, selectedFixtureIds]);
	const mediaIds = useMemo(
		() =>
			selectedFixtureIdsSupportingAttribute(
				fixtures,
				selectedFixtureIds,
				MEDIA_COLOR_ATTRIBUTES,
			),
		[fixtures, selectedFixtureIds],
	);
	return {
		lane,
		ready: Boolean(lane) && (view?.ready ?? false),
		fixtureIds: selectedFixtureIds,
		groupId,
		timing:
			lane === "preload"
				? parameterValueTiming(fadeMillis)
				: immediateParameterTiming(),
		descriptors,
		colorFixtureIds,
		variant: colorDialogVariant(selectedFixtureIds, mediaIds),
		values: (view?.fixtureValues ?? EMPTY) as readonly ColorValueEntry[],
		writers: { normal, preload },
	};
}

const EMPTY: readonly ColorValueEntry[] = [];
