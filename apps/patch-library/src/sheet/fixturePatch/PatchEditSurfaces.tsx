import {
	ModalRegistration,
	ModalTitleBar,
	NumberField,
	Select,
	TextInput,
} from "@tosklight/ui";
import { fixtureDefinitionKey } from "../fixtureProfileModel";
import { usePatchController } from "./controller";
import { saveEdit, saveSplitEdit } from "./editSave";
import { cancelEdit, requestFixtureEditClose } from "./editSession";
import { FixtureAddressScreen } from "./FixtureAddressScreen";
import {
	closeMultipatchEdit,
	requestMultipatchEditClose,
	saveMultipatchEdit,
} from "./multipatchActions";
import { definitionSplits } from "./patchModel";

export function MultipatchVectorDialog() {
	const controller = usePatchController();
	const edit = controller.ui.multipatchEdit;
	if (!edit || edit.kind === "address") return null;
	const policy = edit.kind === "invert_pan" || edit.kind === "invert_tilt";
	const close = () => requestMultipatchEditClose(controller);
	return (
		<ModalRegistration onClose={close}>
			<div className="stacked-modal-layer">
				<section className="nested-modal patch-edit-modal">
					<ModalTitleBar
						title={`Set multi-patch ${
							policy
								? edit.kind === "invert_pan"
									? "Invert Pan"
									: "Invert Tilt"
								: vectorEditTitle(
										edit.kind as "location" | "rotation",
										edit.axis,
									)
						}`}
						accept={{
							id: "set",
							label: "Set",
							variant: "primary",
							onPress: () => void saveMultipatchEdit(controller),
						}}
						closeLabel={`Cancel multi-patch ${edit.kind}`}
						onClose={close}
					/>
					<EditError />
					{policy ? (
						<PolicySelect
							label={
								edit.kind === "invert_pan" ? "Pan direction" : "Tilt direction"
							}
							falseLabel="Normal"
							trueLabel="Inverted"
						/>
					) : (
						<VectorInputs
							kind={edit.kind as "location" | "rotation"}
							axis={edit.axis}
						/>
					)}
				</section>
			</div>
		</ModalRegistration>
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
	const close = () => closeMultipatchEdit(controller);
	return (
		<ModalRegistration onClose={close}>
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
					onCancel={close}
					onConfirm={() => void saveMultipatchEdit(controller)}
				/>
			</div>
		</ModalRegistration>
	);
}

export function FixtureEditDialog() {
	const controller = usePatchController();
	const { edit } = controller.ui;
	if (!edit || !controller.data.selected || edit === "address") return null;
	const close = () => requestFixtureEditClose(controller);
	return (
		<ModalRegistration onClose={close}>
			<div className="stacked-modal-layer">
				<section className="nested-modal patch-edit-modal">
					<ModalTitleBar
						title={`Set fixture ${
							edit === "location" || edit === "rotation"
								? vectorEditTitle(edit, controller.ui.editAxis ?? undefined)
								: editTitle(edit)
						}`}
						accept={
							edit === "name"
								? undefined
								: {
										id: "set",
										label: "Set",
										variant: "primary",
										onPress: () => saveEdit(controller),
									}
						}
						closeLabel={`Cancel fixture ${edit}`}
						onClose={close}
					/>
					<EditError />
					<FixtureEditFields />
				</section>
			</div>
		</ModalRegistration>
	);
}

function FixtureEditFields() {
	const controller = usePatchController();
	const { edit, editText } = controller.ui;
	if (edit === "name" || edit === "note")
		return (
			<TextInput
				clearable
				autoFocus
				aria-label={edit === "note" ? "Fixture note" : "Fixture name"}
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
	if (edit === "bracket_angle")
		return (
			<NumberField
				autoFocus
				label="Bracket angle (°)"
				min={-180}
				max={180}
				step={1}
				allowDecimal
				value={editText}
				onChange={(event) => controller.ui.setEditText(event.target.value)}
			/>
		);
	if (edit === "shaper_angle")
		return (
			<NumberField
				autoFocus
				label="Shaper / barn door angle (°), empty for none"
				min={-180}
				max={180}
				step={1}
				allowDecimal
				value={editText}
				onChange={(event) => controller.ui.setEditText(event.target.value)}
			/>
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
	if (edit === "group_masters" || edit === "grand_master")
		return (
			<>
				<PolicySelect
					label={edit === "group_masters" ? "Group Masters" : "Grand Master"}
					falseLabel="Ignored"
					trueLabel="Controlled"
				/>
				{editText === "false" && (
					<p className="patch-policy-warning" role="alert">
						This fixture can remain live while the{" "}
						{edit === "group_masters"
							? "Group Masters are reduced."
							: "Grand Master is reduced."}
					</p>
				)}
			</>
		);
	if (edit === "invert_pan" || edit === "invert_tilt")
		return (
			<PolicySelect
				label={edit === "invert_pan" ? "Pan direction" : "Tilt direction"}
				falseLabel="Normal"
				trueLabel="Inverted"
			/>
		);
	if (edit === "location" || edit === "rotation")
		return (
			<VectorInputs kind={edit} axis={controller.ui.editAxis ?? undefined} />
		);
	if (edit === "mode") return <ModeField />;
	return null;
}

function PolicySelect({
	label,
	falseLabel,
	trueLabel,
}: {
	label: string;
	falseLabel: string;
	trueLabel: string;
}) {
	const controller = usePatchController();
	return (
		// biome-ignore lint/a11y/noLabelWithoutControl: Select renders its native control inside this label.
		<label>
			{label}
			<Select
				autoFocus
				aria-label={`${label} value`}
				value={controller.ui.editText}
				onChange={(event) => controller.ui.setEditText(event.target.value)}
			>
				<option value="true">{trueLabel}</option>
				<option value="false">{falseLabel}</option>
			</Select>
		</label>
	);
}

function VectorInputs({
	kind,
	axis,
}: {
	kind: "location" | "rotation";
	axis?: "x" | "y" | "z";
}) {
	const controller = usePatchController();
	const axes = axis ? ([axis] as const) : (["x", "y", "z"] as const);
	return (
		<div className="vector-inputs">
			{axes.map((entry) => (
				<NumberField
					key={entry}
					autoFocus={Boolean(axis)}
					label={`${entry.toUpperCase()} ${kind === "location" ? "(m)" : "(°)"}`}
					allowDecimal
					value={
						kind === "location"
							? controller.ui.vector[entry] / 1000
							: controller.ui.vector[entry]
					}
					onChange={(event) =>
						controller.ui.setVector({
							...controller.ui.vector,
							[entry]:
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

function vectorEditTitle(
	kind: "location" | "rotation",
	axis?: "x" | "y" | "z",
) {
	return axis ? `${kind} ${axis.toUpperCase()}` : kind;
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
	if (controller.ui.edit !== "address" || controller.ui.pending || !selected)
		return null;
	const close = () => cancelEdit(controller);
	return (
		<ModalRegistration onClose={close}>
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
					onCancel={close}
					onConfirm={() =>
						definitionSplits(selected.definition).length > 1
							? saveSplitEdit(controller)
							: saveEdit(controller)
					}
				/>
			</div>
		</ModalRegistration>
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
	if (edit === "group_masters") return "Group Masters";
	if (edit === "grand_master") return "Grand Master";
	if (edit === "invert_pan") return "Invert Pan";
	if (edit === "invert_tilt") return "Invert Tilt";
	if (edit === "bracket_angle") return "Bracket angle";
	if (edit === "shaper_angle") return "Shaper angle";
	if (edit === "note") return "note";
	return edit;
}
