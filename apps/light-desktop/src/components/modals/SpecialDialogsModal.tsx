import { Button, ModalPortal } from "@tosklight/ui";
import { useMemo, useState } from "react";
import { useProgrammerFadeMillis } from "../../features/configuration/ConfigurationState";
import { useSelectedPatchedFixtures } from "../../features/patch/PatchState";
import {
	normalizedFixtureMutations,
	programmerValuesMutationKey,
	useProgrammerValuesMutationQueue,
} from "../../features/programmerValues/useProgrammerValuesMutationQueue";
import { useProgrammingSelectionView } from "../../features/programmingInteraction/ProgrammingInteractionView";
import { useApp } from "../../state/AppContext";
import { selectedFixtureIdsSupportingAttribute } from "./specialColor";
import {
	availableSpecialDialogAttributes,
	BeamShapersDialog,
	beamAttributesForFamily,
} from "./specialDialogs/beamShapers";
import { ColorDialog, useColorDialog } from "./specialDialogs/color";
import { ControlDialog } from "./specialDialogs/control";
import { PositionDialog, usePositionDialog } from "./specialDialogs/position";

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
	const [beamPage, setBeamPage] = useState(0);
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

	const close = () =>
		dispatch({ type: "SET_MODAL", modal: "specialDialogsOpen", value: false });

	const apply = async (attribute: string, value: number) => {
		const mutations = normalizedFixtureMutations(
			selectedFixtureIds.map((fixtureId) => ({
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

	if (!state.specialDialogsOpen) return null;
	const beamAttributes =
		family === "Beam" || family === "Shapers"
			? beamAttributesForFamily(available, family)
			: [];

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
						family === "Position" ? "position-special-dialog" : ""
					}`}
				>
					<Button className="modal-close" onClick={close}>
						×
					</Button>
					<h2>{family} · Special Dialog</h2>
					<p>{selectedFixtureIds.length} fixtures selected</p>
					{!valueWrites.canWrite && (
						<p className="modal-status">Programmer values loading…</p>
					)}
					<div className="special-dialog-content">
						{family === "Position" && <PositionDialog {...positionDialog} />}
						{family === "Color" && (
							<ColorDialog {...colorDialog} shiftArmed={state.shiftArmed} />
						)}
						{(family === "Beam" || family === "Shapers") && (
							<BeamShapersDialog
								attributes={beamAttributes}
								family={family}
								page={beamPage}
								setPage={setBeamPage}
								disabled={!valueWrites.canWrite}
								apply={apply}
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
