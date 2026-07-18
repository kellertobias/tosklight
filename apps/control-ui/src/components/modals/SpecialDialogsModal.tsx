import { useMemo, useState } from "react";
import { useServer } from "../../api/ServerContext";
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

export function SpecialDialogsModal() {
	const { state, dispatch } = useApp();
	const server = useServer();
	const [beamPage, setBeamPage] = useState(0);
	const [dynamicSpeed, setDynamicSpeed] = useState(30);
	const family = state.specialDialogFamily;
	const selectedFixtureKey = server.selectedFixtures.join("\u0000");
	const positionDialog = usePositionDialog(
		state.specialDialogsOpen && family === "Position",
		selectedFixtureKey,
	);
	const colorDialog = useColorDialog(state.shiftArmed);
	const available = useMemo(
		() =>
			availableSpecialDialogAttributes(
				server.patch?.fixtures ?? [],
				server.selectedFixtures,
			),
		[server.patch, server.selectedFixtures],
	);

	const close = () =>
		dispatch({ type: "SET_MODAL", modal: "specialDialogsOpen", value: false });

	const apply = async (attribute: string, value: number) => {
		const actions = server.selectedFixtures.map((fixtureId) => ({
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
					<p>{server.selectedFixtures.length} fixtures selected</p>
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
						{family === "Control" && <ControlDialog />}
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
