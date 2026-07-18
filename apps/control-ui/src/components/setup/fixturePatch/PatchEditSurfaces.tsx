import {
	Button,
	ModalTitleBar,
	NumberField,
	Select,
	TextInput,
} from "../../common";
import { fixtureDefinitionKey, maxRaw } from "../fixtureProfileModel";
import { usePatchController } from "./controller";
import { saveEdit, saveHighlightEdit, saveSplitEdit } from "./editSave";
import { cancelEdit, requestFixtureEditClose } from "./editSession";
import { FixtureAddressScreen } from "./FixtureAddressScreen";
import {
	closeMultipatchEdit,
	requestMultipatchEditClose,
	saveMultipatchEdit,
} from "./multipatchActions";
import { definitionModeChannels, definitionSplits } from "./patchModel";

export function MultipatchVectorDialog() {
	const controller = usePatchController();
	const edit = controller.ui.multipatchEdit;
	if (!edit || edit.kind === "address") return null;
	return (
		<div className="stacked-modal-layer">
			<section className="nested-modal patch-edit-modal">
				<ModalTitleBar
					title={`Set multi-patch ${edit.kind}`}
					actions={
						<Button
							className="primary"
							onClick={() => void saveMultipatchEdit(controller)}
						>
							Set
						</Button>
					}
					closeLabel={`Cancel multi-patch ${edit.kind}`}
					onClose={() => requestMultipatchEditClose(controller)}
				/>
				<EditError />
				<VectorInputs kind={edit.kind} />
			</section>
		</div>
	);
}

export function MultipatchAddressDialog() {
	const controller = usePatchController();
	const {
		multipatchAddressFixture: fixture,
		multipatchAddressInstance: instance,
	} = controller.data;
	if (controller.ui.multipatchEdit?.kind !== "address" || !fixture || !instance)
		return null;
	return (
		<div className="stacked-modal-layer fixture-address-layer">
			<FixtureAddressScreen
				fixture={fixture}
				instance={instance}
				fixtures={controller.data.all}
				initialSplit={null}
				singleValue={controller.ui.editText}
				splitValues={controller.ui.editSplitDrafts}
				error={controller.ui.editError}
				onSingleValue={controller.ui.setEditText}
				onSplitValues={controller.ui.setEditSplitDrafts}
				onCancel={() => closeMultipatchEdit(controller)}
				onConfirm={() => void saveMultipatchEdit(controller)}
			/>
		</div>
	);
}

export function FixtureEditDialog() {
	const controller = usePatchController();
	const { edit } = controller.ui;
	if (!edit || !controller.data.selected || edit === "address") return null;
	return (
		<div className="stacked-modal-layer">
			<section className="nested-modal patch-edit-modal">
				<ModalTitleBar
					title={`Set fixture ${editTitle(edit)}`}
					actions={
						edit === "name" ? undefined : (
							<Button
								className="primary"
								onClick={() =>
									edit === "highlight"
										? saveHighlightEdit(controller)
										: saveEdit(controller)
								}
							>
								Set
							</Button>
						)
					}
					closeLabel={`Cancel fixture ${edit}`}
					onClose={() => requestFixtureEditClose(controller)}
				/>
				<EditError />
				<FixtureEditFields />
			</section>
		</div>
	);
}

function FixtureEditFields() {
	const controller = usePatchController();
	const { edit, editText } = controller.ui;
	if (edit === "name")
		return (
			<TextInput
				clearable
				autoFocus
				aria-label="Fixture name"
				value={editText}
				onChange={(event) => controller.ui.setEditText(event.target.value)}
				onKeyboardCommit={(value) => saveEdit(controller, value)}
			/>
		);
	if (edit === "mib")
		return (
			// biome-ignore lint/a11y/noLabelWithoutControl: Select renders its native control inside this label.
			<label>
				Move in Black
				<Select
					autoFocus
					aria-label="Move in Black value"
					value={editText}
					onChange={(event) => controller.ui.setEditText(event.target.value)}
				>
					<option value="true">Enabled</option>
					<option value="false">Disabled</option>
				</Select>
			</label>
		);
	if (edit === "mib_delay")
		return (
			<NumberField
				autoFocus
				label="MIB Delay (s)"
				min={0}
				step={0.1}
				allowDecimal
				value={editText}
				onChange={(event) => controller.ui.setEditText(event.target.value)}
			/>
		);
	if (edit === "highlight") return <HighlightFields />;
	if (edit === "location" || edit === "rotation")
		return <VectorInputs kind={edit} />;
	if (edit === "mode") return <ModeField />;
	return null;
}

function HighlightFields() {
	const controller = usePatchController();
	const selected = controller.data.selected;
	if (!selected) return null;
	return (
		<div className="fixture-highlight-look">
			<p>
				Blank values inherit the profile Highlight raw value. Overrides belong
				to this fixture and remain unchanged when its address changes.
			</p>
			{definitionModeChannels(selected.definition).map((channel) => (
				<NumberField
					key={channel.id}
					label={`${channel.attribute} highlight raw (profile ${channel.highlight_raw})`}
					min={0}
					max={maxRaw(channel.resolution)}
					value={controller.ui.highlightDrafts[channel.id] ?? ""}
					onChange={(event) =>
						controller.ui.setHighlightDrafts((current) => ({
							...current,
							[channel.id]: event.target.value,
						}))
					}
				/>
			))}
		</div>
	);
}

function VectorInputs({ kind }: { kind: "location" | "rotation" }) {
	const controller = usePatchController();
	return (
		<div className="vector-inputs">
			{(["x", "y", "z"] as const).map((axis) => (
				<NumberField
					key={axis}
					label={`${axis.toUpperCase()} ${kind === "location" ? "(m)" : ""}`}
					allowDecimal
					value={
						kind === "location"
							? controller.ui.vector[axis] / 1000
							: controller.ui.vector[axis]
					}
					onChange={(event) =>
						controller.ui.setVector({
							...controller.ui.vector,
							[axis]:
								kind === "location"
									? Math.round(Number(event.target.value) * 1000)
									: Number(event.target.value),
						})
					}
				/>
			))}
		</div>
	);
}

function ModeField() {
	const controller = usePatchController();
	const family = controller.data.selectedModeFamily;
	if (!family) return null;
	return (
		// biome-ignore lint/a11y/noLabelWithoutControl: Select renders its native control inside this label.
		<label>
			Product / mode
			<Select
				aria-label="Product / mode"
				value={controller.ui.definitionKey}
				onChange={(event) => controller.ui.setDefinitionKey(event.target.value)}
			>
				{family.modes.map((mode) => (
					<option
						value={fixtureDefinitionKey(mode)}
						key={fixtureDefinitionKey(mode)}
					>
						{mode.mode} · {mode.footprint}ch
					</option>
				))}
			</Select>
		</label>
	);
}

export function FixtureAddressDialog() {
	const controller = usePatchController();
	const selected = controller.data.selected;
	if (controller.ui.edit !== "address" || !selected) return null;
	return (
		<div className="stacked-modal-layer fixture-address-layer">
			<FixtureAddressScreen
				fixture={selected}
				fixtures={controller.data.all}
				initialSplit={controller.ui.editingSplit}
				singleValue={controller.ui.editText}
				splitValues={controller.ui.editSplitDrafts}
				error={controller.ui.editError}
				onSingleValue={controller.ui.setEditText}
				onSplitValues={controller.ui.setEditSplitDrafts}
				onCancel={() => cancelEdit(controller)}
				onConfirm={() =>
					definitionSplits(selected.definition).length > 1
						? saveSplitEdit(controller)
						: saveEdit(controller)
				}
			/>
		</div>
	);
}

function EditError() {
	const error = usePatchController().ui.editError;
	return error ? (
		<p className="patch-status" role="alert">
			{error}
		</p>
	) : null;
}

function editTitle(
	edit: NonNullable<ReturnType<typeof usePatchController>["ui"]["edit"]>,
) {
	if (edit === "mib") return "MIB";
	if (edit === "mib_delay") return "MIB Delay";
	if (edit === "highlight") return "Highlight Look";
	return edit;
}
