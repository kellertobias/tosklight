import { useMemo } from "react";
import { useProgrammerFadeMillis } from "../../../../features/configuration/ConfigurationState";
import { useSelectedPatchedFixtures } from "../../../../features/patch/PatchState";
import {
	normalizedFixtureMutations,
	programmerValuesMutationKey,
	useProgrammerValuesMutationQueue,
} from "../../../../features/programmerValues/useProgrammerValuesMutationQueue";
import { useProgrammingSelectionView } from "../../../../features/programmingInteraction/ProgrammingInteractionView";
import type { AppState } from "../../../../types";
import { useParameterPreloadValues } from "../../../control/parameterControls/useParameterPreloadValues";
import { useParameterProgrammerValues } from "../../../control/parameterControls/useParameterProgrammerValues";
import { selectedFixtureIdsSupportingAttribute } from "../../specialColor";
import {
	availableSpecialDialogAttributes,
	isShaperDialogAttribute,
} from "../beamShapers";
import { useColorDialog } from "../color";
import { usePositionDialog } from "../position";
import type { ShaperAttributeValue } from "../shapers";

/**
 * The legacy (contract 0) Special Dialog state, moved unchanged out of `SpecialDialogsModal`.
 * It stays mounted with the modal exactly as before — the Color dialog keeps its picker state
 * across open/close and the Position dialog only activates while Position is open — so the
 * legacy dialogs behave byte-for-byte as they did before the registry split.
 */

const EMPTY_FIXTURE_IDS: readonly string[] = [];

function useSelectionHost(open: boolean, family: string) {
	const selection = useProgrammingSelectionView(open);
	// The Media pane owns its own programmer writes.
	const valueWrites = useProgrammerValuesMutationQueue(
		open && family !== "Media",
	);
	const selectedFixtureIds = selection?.selected ?? EMPTY_FIXTURE_IDS;
	const selectedFixtures = useSelectedPatchedFixtures(selectedFixtureIds, open);
	return { valueWrites, selectedFixtureIds, selectedFixtures };
}

function useValueDialogs(
	state: Pick<AppState, "specialDialogsOpen" | "specialDialogFamily" | "shiftArmed">,
	host: ReturnType<typeof useSelectionHost>,
) {
	const { selectedFixtureIds, selectedFixtures, valueWrites } = host;
	const positionDialog = usePositionDialog(
		state.specialDialogsOpen && state.specialDialogFamily === "Position",
		selectedFixtureIds,
		valueWrites,
	);
	const tintFixtureIds = useMemo(
		() =>
			selectedFixtureIdsSupportingAttribute(
				selectedFixtures,
				selectedFixtureIds,
				["color.tint", "fixture.tint"],
			),
		[selectedFixtures, selectedFixtureIds],
	);
	const grayscaleFixtureIds = useMemo(
		() =>
			selectedFixtureIdsSupportingAttribute(
				selectedFixtures,
				selectedFixtureIds,
				["media.grayscale"],
			),
		[selectedFixtures, selectedFixtureIds],
	);
	const colorDialog = useColorDialog(
		selectedFixtureIds,
		state.shiftArmed,
		valueWrites,
		tintFixtureIds,
		grayscaleFixtureIds,
	);
	return { positionDialog, colorDialog };
}

function useAttributeDialogs(
	state: Pick<AppState, "specialDialogsOpen" | "specialDialogFamily">,
	host: ReturnType<typeof useSelectionHost>,
) {
	const { selectedFixtureIds, selectedFixtures, valueWrites } = host;
	const family = state.specialDialogFamily;
	const available = useMemo(
		() =>
			availableSpecialDialogAttributes(selectedFixtures, selectedFixtureIds),
		[selectedFixtures, selectedFixtureIds],
	);
	const programmerValues = useParameterProgrammerValues(
		selectedFixtureIds,
		null,
		state.specialDialogsOpen &&
			family === "Shapers" &&
			valueWrites.route !== "preload",
	);
	const preloadValues = useParameterPreloadValues(
		selectedFixtureIds,
		null,
		state.specialDialogsOpen &&
			family === "Shapers" &&
			valueWrites.route === "preload",
	);
	const activeProgrammerValues =
		valueWrites.route === "preload" ? preloadValues : programmerValues;
	const shaperValues = useMemo(() => {
		const result: Record<string, ShaperAttributeValue> = {};
		for (const attribute of available) {
			if (!isShaperDialogAttribute(attribute)) continue;
			const entries =
				activeProgrammerValues?.fixtureValues.filter(
					(entry) =>
						entry.attribute === attribute && entry.value.kind === "normalized",
				) ?? [];
			const normalized = entries.flatMap((entry) =>
				entry.value.kind === "normalized" ? [entry.value.value] : [],
			);
			if (!normalized.length) continue;
			result[attribute] = {
				value:
					normalized.reduce((sum, value) => sum + value, 0) / normalized.length,
				mixed: normalized.some((value) => value !== normalized[0]),
			};
		}
		return result;
	}, [activeProgrammerValues, available]);
	return { available, shaperValues };
}

export function useLegacySpecialDialogHost(
	state: Pick<
		AppState,
		"specialDialogsOpen" | "specialDialogFamily" | "shiftArmed"
	>,
) {
	const programmerFadeMillis = useProgrammerFadeMillis() ?? undefined;
	const host = useSelectionHost(
		state.specialDialogsOpen,
		state.specialDialogFamily,
	);
	const values = useValueDialogs(state, host);
	const attributes = useAttributeDialogs(state, host);
	const { selectedFixtures, selectedFixtureIds, valueWrites } = host;
	const apply = async (attribute: string, value: number) => {
		const fixtureIds = selectedFixtureIdsSupportingAttribute(
			selectedFixtures,
			selectedFixtureIds,
			[attribute],
		);
		const mutations = normalizedFixtureMutations(
			fixtureIds.map((fixtureId) => ({
				fixtureId,
				attribute,
				value,
			})),
			programmerFadeMillis,
		);
		await valueWrites.submitLatest(
			programmerValuesMutationKey(mutations),
			mutations,
		);
	};
	return {
		...host,
		...values,
		...attributes,
		shiftArmed: state.shiftArmed,
		apply,
	};
}

export type LegacySpecialDialogHost = ReturnType<
	typeof useLegacySpecialDialogHost
>;
