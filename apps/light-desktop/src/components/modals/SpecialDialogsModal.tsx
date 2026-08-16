import { ModalPortal, ModalTitleBar } from "@tosklight/ui";
import { useMemo } from "react";
import { useProgrammerFadeMillis } from "../../features/configuration/ConfigurationState";
import { useSelectedPatchedFixtures } from "../../features/patch/PatchState";
import {
	normalizedFixtureMutations,
	programmerValuesMutationKey,
	useProgrammerValuesMutationQueue,
} from "../../features/programmerValues/useProgrammerValuesMutationQueue";
import { useProgrammingSelectionView } from "../../features/programmingInteraction/ProgrammingInteractionView";
import { useApp } from "../../state/AppContext";
import {
	type IndexedPresetChoice,
	indexedPresetChoices,
} from "../control/parameterControls/indexedPresetChoices";
import { useParameterPreloadValues } from "../control/parameterControls/useParameterPreloadValues";
import { useParameterProgrammerValues } from "../control/parameterControls/useParameterProgrammerValues";
import { selectedFixtureIdsSupportingAttribute } from "./specialColor";
import {
	availableSpecialDialogAttributes,
	beamAttributesForFamily,
} from "./specialDialogs/beamShapers";
import { ColorDialog, useColorDialog } from "./specialDialogs/color";
import { ControlDialog } from "./specialDialogs/control";
import { MediaPlayModeDialog, playModeMutations } from "./specialDialogs/media";
import { PositionDialog, usePositionDialog } from "./specialDialogs/position";
import {
	type ShaperAttributeValue,
	ShapersDialog,
} from "./specialDialogs/shapers";

export {
	type AuthoredFixtureControlChoice,
	type CompatibleFixtureControlAction,
	compatibleAuthoredControlActions,
	compatibleSpecialDialogActions,
} from "./specialDialogs/control";

const EMPTY_FIXTURE_IDS: readonly string[] = [];

export function SpecialDialogsModal() {
	const { state, dispatch } = useApp();
	const programmerFadeMillis = useProgrammerFadeMillis() ?? undefined;
	const family = state.specialDialogFamily;
	const selection = useProgrammingSelectionView(state.specialDialogsOpen);
	const valueWrites = useProgrammerValuesMutationQueue(
		state.specialDialogsOpen,
	);
	const selectedFixtureIds = selection?.selected ?? EMPTY_FIXTURE_IDS;
	const positionDialog = usePositionDialog(
		state.specialDialogsOpen && family === "Position",
		selectedFixtureIds,
		valueWrites,
	);
	const selectedFixtures = useSelectedPatchedFixtures(
		selectedFixtureIds,
		state.specialDialogsOpen,
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
	const available = useMemo(
		() =>
			availableSpecialDialogAttributes(selectedFixtures, selectedFixtureIds),
		[selectedFixtures, selectedFixtureIds],
	);
	const programmerValues = useParameterProgrammerValues(
		selectedFixtureIds,
		null,
		state.specialDialogsOpen &&
			(family === "Shapers" || family === "Media") &&
			valueWrites.route !== "preload",
	);
	const preloadValues = useParameterPreloadValues(
		selectedFixtureIds,
		null,
		state.specialDialogsOpen &&
			(family === "Shapers" || family === "Media") &&
			valueWrites.route === "preload",
	);
	const activeProgrammerValues =
		valueWrites.route === "preload" ? preloadValues : programmerValues;
	const shaperValues = useMemo(() => {
		const result: Record<string, ShaperAttributeValue> = {};
		for (const attribute of available) {
			if (!attribute.startsWith("shaper.")) continue;
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
	const playModeChoices = useMemo(
		() =>
			indexedPresetChoices(
				selectedFixtures,
				selectedFixtureIds,
				"media.play_mode",
			),
		[selectedFixtures, selectedFixtureIds],
	);
	const playModeValue = useMemo(() => {
		const values =
			activeProgrammerValues?.fixtureValues.flatMap((entry) =>
				entry.attribute === "media.play_mode" && entry.value.kind === "discrete"
					? [entry.value.value]
					: [],
			) ?? [];
		return {
			value: values[0] ?? null,
			mixed: values.some((value) => value !== values[0]),
		};
	}, [activeProgrammerValues]);

	const close = () =>
		dispatch({ type: "SET_MODAL", modal: "specialDialogsOpen", value: false });

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
	const applyPlayMode = async (choice: IndexedPresetChoice) => {
		const mutations = playModeMutations(choice, programmerFadeMillis);
		await valueWrites.submitBarrier(mutations);
	};

	if (!state.specialDialogsOpen) return null;
	const shaperAttributes =
		family === "Shapers" ? beamAttributesForFamily(available, "Shapers") : [];

	return (
		<ModalPortal onClose={close}>
			<div
				className="modal-backdrop"
				onPointerDown={(event) => {
					if (event.target === event.currentTarget) close();
				}}
			>
				<section
					className={`modal-card special-dialog-card ${
						family === "Position"
							? "position-special-dialog"
							: family === "Shapers"
								? "shapers-special-dialog-card"
								: ""
					}`}
				>
					<ModalTitleBar title={`${family} · Special Dialog`} onClose={close} />
					<p>{selectedFixtureIds.length} fixtures selected</p>
					{!valueWrites.canWrite && (
						<p className="modal-status">Programmer values loading…</p>
					)}
					<div className="special-dialog-content">
						{family === "Position" && <PositionDialog {...positionDialog} />}
						{family === "Color" && (
							<ColorDialog {...colorDialog} shiftArmed={state.shiftArmed} />
						)}
						{family === "Shapers" && (
							<ShapersDialog
								attributes={shaperAttributes}
								values={shaperValues}
								disabled={!valueWrites.canWrite}
								apply={apply}
							/>
						)}
						{family === "Media" && (
							<MediaPlayModeDialog
								choices={playModeChoices}
								value={playModeValue.value}
								mixed={playModeValue.mixed}
								disabled={!valueWrites.canWrite}
								apply={applyPlayMode}
							/>
						)}
						{family === "Control" && (
							<ControlDialog selectedFixtureIds={selectedFixtureIds} />
						)}
					</div>
				</section>
			</div>
		</ModalPortal>
	);
}
