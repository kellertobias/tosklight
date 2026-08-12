import {
	Button,
	ModalRegistration,
	ModalTitleBar,
	NumberField,
	Select,
	TextInput,
} from "@tosklight/ui";
import { ModalNumberEditor } from "@tosklight/ui/input";
import { fixtureDefinitionKey } from "../fixtureProfileModel";
import {
	allowedCombinedPolicyChoices,
	type CombinedPolicyChoice,
	combinedPolicyValues,
} from "./combinedPolicy";
import { usePatchController } from "./controller";
import {
	saveEdit,
	saveSplitEdit,
	saveVectorAxisInput,
	saveVectorSpread,
} from "./editSave";
import { cancelEdit, requestFixtureEditClose } from "./editSession";
import { FixtureAddressScreen } from "./FixtureAddressScreen";
import {
	closeMultipatchEdit,
	requestMultipatchEditClose,
	saveMultipatchEdit,
	saveMultipatchVectorInput,
} from "./multipatchActions";
import { definitionSplits, fixturePolicyApplicability } from "./patchModel";

export function MultipatchVectorDialog() {
	const controller = usePatchController();
	const edit = controller.ui.multipatchEdit;
	if (!edit || edit.kind === "address") return null;
	const policy = edit.kind === "pan_tilt";
	const close = () => requestMultipatchEditClose(controller);
	if (!policy && edit.axis) {
		const label = `${edit.kind === "location" ? "Location" : "Rotation"} ${edit.axis.toUpperCase()} (${edit.kind === "location" ? "meter" : "degree"})`;
		return (
			<ModalNumberEditor
				ariaLabel={label}
				title={label}
				value={controller.ui.editText}
				onChange={controller.ui.setEditText}
				onSubmit={(value) =>
					void saveMultipatchVectorInput(
						controller,
						value ?? controller.ui.editText,
					)
				}
				onClose={close}
				allowDecimal
				allowThrough={(edit.physicalTargets?.length ?? 0) > 1}
				unit={edit.kind === "location" ? "meter" : "degree"}
			/>
		);
	}
	return (
		<ModalRegistration onClose={close}>
			<div className="stacked-modal-layer">
				<section className="nested-modal patch-edit-modal">
					<ModalTitleBar
						title={`Set multi-patch ${
							policy
								? "Pan / Tilt"
								: vectorEditTitle(
										edit.kind as "location" | "rotation",
										edit.axis,
									)
						}`}
						actions={
							<Button
								className="primary"
								onClick={() => void saveMultipatchEdit(controller)}
							>
								Set
							</Button>
						}
						closeLabel={`Cancel multi-patch ${edit.kind}`}
						onClose={close}
					/>
					<EditError />
					{policy ? (
						<CombinedPolicySelect kind="pan_tilt" />
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
	if ((edit === "location" || edit === "rotation") && controller.ui.editAxis) {
		const axis = controller.ui.editAxis;
		const label = `${edit === "location" ? "Location" : "Rotation"} ${axis.toUpperCase()} (${edit === "location" ? "meter" : "degree"})`;
		return (
			<ModalNumberEditor
				ariaLabel={label}
				title={label}
				value={controller.ui.editText}
				onChange={controller.ui.setEditText}
				onSubmit={(value) =>
					void saveVectorAxisInput(
						controller,
						edit,
						axis,
						value ?? controller.ui.editText,
					)
				}
				onClose={close}
				allowDecimal
				allowThrough={(controller.selection.orderedFixtureIds?.length ?? 0) > 1}
				unit={edit === "location" ? "meter" : "degree"}
			/>
		);
	}
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
						actions={
							edit === "name" ? undefined : (
								<Button
									className="primary"
									onClick={() => saveEdit(controller)}
								>
									Set
								</Button>
							)
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
			<div>
				<TextInput
					autoFocus
					aria-label="MIB value: Off or non-negative seconds"
					value={editText}
					onChange={(event) => controller.ui.setEditText(event.target.value)}
				/>
				<small>Enter Off or a non-negative delay in seconds. 0 means on.</small>
			</div>
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
	if (edit === "internal_bindings") return <InternalBindingsFields />;
	if (edit === "masters" || edit === "pan_tilt")
		return <CombinedPolicySelect kind={edit} />;
	if (edit === "location" || edit === "rotation")
		return (
			<VectorInputs kind={edit} axis={controller.ui.editAxis ?? undefined} />
		);
	if (edit === "mode") return <ModeField />;
	return null;
}

function InternalBindingsFields() {
	const controller = usePatchController();
	let draft = { library: "", output: "" };
	try {
		draft = JSON.parse(controller.ui.editText) as typeof draft;
	} catch {
		// The editor owns this private draft format and recovers to empty fields.
	}
	const update = (key: keyof typeof draft, value: string) =>
		controller.ui.setEditText(JSON.stringify({ ...draft, [key]: value }));
	return (
		<div className="vector-inputs">
			<label>
				Audio library binding
				<TextInput
					autoFocus
					aria-label="Logical audio library binding"
					value={draft.library}
					onChange={(event) => update("library", event.target.value)}
				/>
			</label>
			<label>
				Audio output binding
				<TextInput
					aria-label="Logical audio output binding"
					value={draft.output}
					onChange={(event) => update("output", event.target.value)}
				/>
			</label>
			<small>
				Portable logical names only. This desk resolves local folders and devices in
				 Setup.
			</small>
		</div>
	);
}

function CombinedPolicySelect({ kind }: { kind: "masters" | "pan_tilt" }) {
	const controller = usePatchController();
	const edit = controller.ui.multipatchEdit;
	const fixture = edit
		? controller.data.all.find(
				(candidate) => candidate.fixture_id === edit.fixtureId,
			)
		: controller.data.selected;
	if (!fixture) return null;
	const applicable = fixturePolicyApplicability(fixture.definition);
	const firstAvailable =
		kind === "masters" ? applicable.groupMasters : applicable.pan;
	const secondAvailable =
		kind === "masters" ? applicable.grandMaster : applicable.tilt;
	const choices = allowedCombinedPolicyChoices(firstAvailable, secondAvailable);
	const labels: Record<CombinedPolicyChoice, string> =
		kind === "masters"
			? {
					none: "Not controlled",
					first: "Group Master",
					second: "Grand Master",
					both: "Both",
				}
			: {
					none: "None",
					first: "Invert Pan",
					second: "Invert Tilt",
					both: "Invert Both",
				};
	const value = controller.ui.editText as CombinedPolicyChoice;
	const values = combinedPolicyValues(value);
	const warning =
		kind === "masters" &&
		((firstAvailable && !values.first) || (secondAvailable && !values.second));
	return (
		<>
			{/* biome-ignore lint/a11y/noLabelWithoutControl: Select renders its native control inside this label. */}
			<label>
				{kind === "masters" ? "Master participation" : "Pan / Tilt inversion"}
				<Select
					autoFocus
					aria-label={
						kind === "masters"
							? "Master participation value"
							: "Pan and Tilt inversion value"
					}
					value={value}
					onChange={(event) => controller.ui.setEditText(event.target.value)}
				>
					{choices.map((choice) => (
						<option key={choice} value={choice}>
							{labels[choice]}
						</option>
					))}
				</Select>
			</label>
			{warning && (
				<p className="patch-policy-warning" role="alert">
					This fixture may remain live while an applicable master is reduced.
				</p>
			)}
		</>
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
					allowThrough={Boolean(
						axis && (controller.selection.orderedFixtureIds?.length ?? 0) > 1,
					)}
					onRangeCommit={(points) =>
						void saveVectorSpread(controller, kind, entry, points)
					}
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
	if (edit === "masters") return "Masters";
	if (edit === "pan_tilt") return "Pan / Tilt";
	if (edit === "bracket_angle") return "Bracket angle";
	if (edit === "shaper_angle") return "Shaper angle";
	if (edit === "internal_bindings") return "Audio bindings";
	return edit;
}
