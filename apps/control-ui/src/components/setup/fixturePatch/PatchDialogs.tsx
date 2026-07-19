import { Button, TextInput } from "../../common";
import { fixtureRange } from "../patchUtils";
import { usePatchController } from "./controller";
import { cancelEdit } from "./editSession";
import {
	createLayer,
	deleteFixture,
	unpatchConflictsAndApply,
	unpatchCurrentFixture,
	unpatchFixtureFromDeleteConfirm,
} from "./fixtureActions";
import { fixtureDisplayId } from "./fixtureIds";
import { closeMultipatchEdit } from "./multipatchActions";
import { closePlacement } from "./placementDraft";

export function PlacementCloseConfirm() {
	const controller = usePatchController();
	if (!controller.ui.placementCloseConfirm) return null;
	return (
		<div className="stacked-modal-layer">
			<section
				className="nested-modal patch-small-modal"
				role="dialog"
				aria-modal="true"
				aria-labelledby="close-add-fixture-title"
			>
				<h3 id="close-add-fixture-title">Close Add Fixture?</h3>
				<p>
					Your changes in Add Fixture have not been applied. Do you really want
					to close?
				</p>
				<footer>
					<Button className="danger" onClick={() => closePlacement(controller)}>
						Yes, close
					</Button>
					<Button onClick={() => controller.ui.setPlacementCloseConfirm(false)}>
						Stay in Add Fixture
					</Button>
				</footer>
			</section>
		</div>
	);
}

export function EditCloseConfirm() {
	const controller = usePatchController();
	const target = controller.ui.editCloseConfirm;
	if (!target) return null;
	return (
		<div className="stacked-modal-layer">
			<section
				className="nested-modal patch-small-modal"
				role="dialog"
				aria-modal="true"
				aria-label="Discard fixture changes?"
			>
				<h3>Discard changes?</h3>
				<p>
					The changed{" "}
					{target === "fixture"
						? controller.ui.edit
						: controller.ui.multipatchEdit?.kind}{" "}
					values have not been saved.
				</p>
				<footer>
					<Button
						className="danger"
						onClick={() => {
							controller.ui.setEditCloseConfirm(null);
							if (target === "fixture") cancelEdit(controller);
							else closeMultipatchEdit(controller);
						}}
					>
						Discard changes
					</Button>
					<Button onClick={() => controller.ui.setEditCloseConfirm(null)}>
						Keep editing
					</Button>
				</footer>
			</section>
		</div>
	);
}

export function DeleteConfirm() {
	const controller = usePatchController();
	const fixture = controller.ui.deleteConfirm;
	if (!fixture) return null;
	return (
		<div className="stacked-modal-layer">
			<section
				className="nested-modal patch-small-modal"
				role="alertdialog"
				aria-modal="true"
				aria-label={`Delete or unpatch ${fixture.name || fixture.definition.name}?`}
			>
				<h3>Delete or unpatch {fixtureDisplayId(fixture)}?</h3>
				<p>
					Delete removes <b>{fixture.name || fixture.definition.name}</b> from
					the show. Unpatch keeps the fixture line and clears its DMX addresses,
					including multi-patch addresses.
				</p>
				<footer>
					<Button
						className="danger"
						autoFocus
						onClick={() => void deleteFixture(controller)}
					>
						Delete fixture
					</Button>
					<Button
						onClick={() => void unpatchFixtureFromDeleteConfirm(controller)}
					>
						Unpatch fixture
					</Button>
					<Button onClick={() => controller.ui.setDeleteConfirm(null)}>
						Abort
					</Button>
				</footer>
			</section>
		</div>
	);
}

export function AddLayerDialog() {
	const controller = usePatchController();
	if (controller.ui.layerModal !== "add") return null;
	return (
		<div className="stacked-modal-layer">
			<section className="nested-modal patch-small-modal">
				<h3>Add layer</h3>
				<TextInput
					clearable
					autoFocus
					aria-label="Layer name"
					value={controller.ui.layerName}
					onChange={(event) => controller.ui.setLayerName(event.target.value)}
					onKeyboardCommit={(value) => void createLayer(controller, value)}
				/>
				<footer>
					<Button onClick={() => controller.ui.setLayerModal(null)}>
						Cancel
					</Button>
					<Button onClick={() => void createLayer(controller)}>
						Add layer
					</Button>
				</footer>
			</section>
		</div>
	);
}

export function PatchConflictDialog() {
	const controller = usePatchController();
	const { pending, blockedBy, editError } = controller.ui;
	if (!pending || !controller.data.selected) return null;
	return (
		<div className="stacked-modal-layer">
			<section
				className="nested-modal conflict-modal"
				role="dialog"
				aria-modal="true"
				aria-label="Patch conflict"
			>
				<h3>Patch conflict</h3>
				{editError && (
					<p className="patch-status" role="alert">
						{editError}
					</p>
				)}
				<p>
					The requested range overlaps {blockedBy.map(conflictLabel).join(", ")}
					.
				</p>
				<footer>
					<Button
						onClick={() => {
							controller.ui.setPending(null);
							controller.ui.setBlockedBy([]);
						}}
					>
						Keep old patch / mode
					</Button>
					<Button onClick={() => void unpatchCurrentFixture(controller)}>
						Unpatch current fixture
					</Button>
					<Button
						className="danger"
						onClick={() => void unpatchConflictsAndApply(controller)}
					>
						Unpatch conflicts and apply
					</Button>
				</footer>
			</section>
		</div>
	);
}

function conflictLabel(fixture: PatchControllerFixture) {
	return `${fixture.name || fixture.definition.name} (${fixtureRange(fixture)?.universe}.${fixtureRange(fixture)?.start}–${fixtureRange(fixture)?.end})`;
}

type PatchControllerFixture = ReturnType<
	typeof usePatchController
>["data"]["all"][number];
