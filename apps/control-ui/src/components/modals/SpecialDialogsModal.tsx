import { useMemo, useState } from "react";
import { useServer } from "../../api/ServerContext";
import { useProgrammingSelectionView } from "../../features/programmingInteraction/ProgrammingInteractionView";
import { useApp } from "../../state/AppContext";
import { Button, ModalPortal } from "../common";
import {
	availableSpecialDialogAttributes,
	BeamShapersDialog,
	beamAttributesForFamily,
} from "./specialDialogs/beamShapers";
import { ColorDialog, useColorDialog } from "./specialDialogs/color";
import { ControlDialog } from "./specialDialogs/control";
import { DynamicsDialog } from "./specialDialogs/dynamics";
import { PositionDialog, usePositionDialog } from "./specialDialogs/position";

export {
	type CompatibleFixtureControlAction,
	compatibleSpecialDialogActions,
} from "./specialDialogs/control";

const EMPTY_FIXTURE_IDS: readonly string[] = [];

export function SpecialDialogsModal() {
	const { state, dispatch } = useApp();
	const server = useServer();
	const [beamPage, setBeamPage] = useState(0);
	const [dynamicSpeed, setDynamicSpeed] = useState(30);
	const family = state.specialDialogFamily;
	const selection = useProgrammingSelectionView(state.specialDialogsOpen);
	const selectedFixtureIds = selection?.selected ?? EMPTY_FIXTURE_IDS;
	const positionDialog = usePositionDialog(
		state.specialDialogsOpen && family === "Position",
		selectedFixtureIds,
	);
	const colorDialog = useColorDialog(selectedFixtureIds, state.shiftArmed);
	const available = useMemo(
		() =>
			availableSpecialDialogAttributes(
				server.patch?.fixtures ?? [],
				selectedFixtureIds,
			),
		[server.patch, selectedFixtureIds],
	);

	const close = () =>
		dispatch({ type: "SET_MODAL", modal: "specialDialogsOpen", value: false });

	const apply = async (attribute: string, value: number) => {
		const actions = selectedFixtureIds.map((fixtureId) => ({
			fixtureId,
			attribute,
		}));
		await Promise.all(
			actions.map((action) =>
				server.setProgrammer(action.fixtureId, action.attribute, value),
			),
		);
	};

	if (!state.specialDialogsOpen) return null;
	const beamAttributes =
		family === "Beam" || family === "Shapers"
			? beamAttributesForFamily(available, family)
			: [];

	return (
		<ModalPortal>
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
								apply={apply}
							/>
						)}
						{family === "Control" && (
							<ControlDialog selectedFixtureIds={selectedFixtureIds} />
						)}
						{family === "Dynamics" && (
							<DynamicsDialog
								speed={dynamicSpeed}
								setSpeed={setDynamicSpeed}
								apply={apply}
							/>
						)}
					</div>
					{server.error && (
						<p className="modal-error" role="alert">
							{server.error}
						</p>
					)}
				</section>
			</div>
		</ModalPortal>
	);
}
