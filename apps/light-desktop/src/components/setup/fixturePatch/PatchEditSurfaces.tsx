import { ErrorAlert } from "@tosklight/ui";
import { PositionCalibrationButton } from "./PositionCalibration";
import { isVisualOnly } from "../patchUtils";
import { positionPointLabel, positionPoints } from "./positionReference";
import { SCENERY_AXES } from "./scenerySize";
import { CHAIN_MODES } from "./sceneryOptions";
import {
	Button,
	ColorPickerField,
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
import { rootProgrammingCorrespondences } from "./replacementProgramming";

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
						<><CombinedPolicySelect kind="pan_tilt" /><PositionCalibrationButton /></>
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
	const replacingPending = controller.ui.replacingFixture && controller.patch.pendingFixtureIds.has(controller.data.selected.fixture_id);
	const close = () => { if (!replacingPending) requestFixtureEditClose(controller); };
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
	const sceneryAxis = SCENERY_AXES.find((entry) => entry.edit === edit);
	if (sceneryAxis) {
		return (
			<ModalNumberEditor
				ariaLabel={`${sceneryAxis.label} (metre)`}
				title={`Set ${sceneryAxis.label.toLowerCase()}`}
				value={controller.ui.editText}
				onChange={controller.ui.setEditText}
				onSubmit={(value) =>
					saveEdit(controller, value ?? controller.ui.editText)
				}
				onClose={close}
				allowDecimal
				unit="meter"
				error={controller.ui.editError}
			/>
		);
	}
	if (edit === "model_scale") {
		return (
			<ModalNumberEditor
				ariaLabel="Scale"
				title="Set scale"
				value={controller.ui.editText}
				onChange={controller.ui.setEditText}
				onSubmit={(value) =>
					saveEdit(controller, value ?? controller.ui.editText)
				}
				onClose={close}
				allowDecimal
				error={controller.ui.editError}
			/>
		);
	}
	if (edit === "crowd_width" || edit === "crowd_depth") {
		const label = edit === "crowd_width" ? "Crowd width" : "Crowd depth";
		return (
			<ModalNumberEditor
				ariaLabel={`${label} (metre)`}
				title={`Set ${label.toLowerCase()}`}
				value={controller.ui.editText}
				onChange={controller.ui.setEditText}
				onSubmit={(value) =>
					saveEdit(controller, value ?? controller.ui.editText)
				}
				onClose={close}
				allowDecimal
				unit="meter"
				error={controller.ui.editError}
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
						accept={
							edit === "name"
								? undefined
								: {
										id: "set",
										label: replacingPending ? "Replacing fixture…" : "Set",
										disabled: replacingPending,
										variant: "primary",
										onPress: () => saveEdit(controller),
									}
						}
						closeLabel={`Cancel fixture ${edit}`}
						closeDisabled={replacingPending}
						onClose={close}
					/>
					<EditError />
					{replacingPending && <p role="status">Validating and applying the replacement. The current fixture remains visible until the authoritative result arrives.</p>}
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
	if (edit === "scenery_colour") return <SceneryColourFields />;
	if (edit === "chain") return <ChainModeFields />;
	if (edit === "position_reference") return <PositionReferenceFields />;
	if (edit === "masters" || edit === "pan_tilt")
		return <><CombinedPolicySelect kind={edit} />{edit === "pan_tilt" && <PositionCalibrationButton />}</>;
	if (edit === "location" || edit === "rotation")
		return (
			<VectorInputs kind={edit} axis={controller.ui.editAxis ?? undefined} />
		);
	if (edit === "mode") return <ModeField />;
	return null;
}

/**
 * A generated Venue object's colour. **Default colour** returns it to its kind's own material —
 * black serge for a curtain, raw aluminium for truss — which is what it was drawn in before.
 */
function SceneryColourFields() {
	const controller = usePatchController();
	const colour = controller.ui.editText;
	return (
		<>
			<ColorPickerField
				label="Colour"
				value={colour || "#101010"}
				onChange={(chosen) => controller.ui.setEditText(chosen.toUpperCase())}
			/>
			<Button active={!colour} onClick={() => controller.ui.setEditText("")}>
				Default colour
			</Button>
			<small>
				{colour
					? `Drawn in ${colour.toUpperCase()}.`
					: "Drawn in the object's own material."}
			</small>
		</>
	);
}

/** How a chain is rigged; each choice stores both of its end fittings. */
function ChainModeFields() {
	const controller = usePatchController();
	return (
		// biome-ignore lint/a11y/noLabelWithoutControl: Select renders its native control inside this label.
		<label>
			Chain
			<Select
				autoFocus
				aria-label="Chain mode"
				value={controller.ui.editText}
				onChange={(event) => controller.ui.setEditText(event.target.value)}
			>
				{CHAIN_MODES.map((option) => (
					<option key={option.value} value={option.value}>
						{option.label}
					</option>
				))}
			</Select>
		</label>
	);
}

/**
 * Which 3D Point this fixture or Venue object follows. **None** places it against the stage. The
 * choice is a point in this show, named by its fixture ID and name; a point is never offered to
 * itself, and a point cannot follow another point.
 */
function PositionReferenceFields() {
	const controller = usePatchController();
	const selected = controller.data.selected;
	const points = positionPoints(controller.data.all).filter(
		(point) => point.fixture_id !== selected?.fixture_id,
	);
	return (
		// biome-ignore lint/a11y/noLabelWithoutControl: Select renders its native control inside this label.
		<label>
			Position Reference
			<Select
				autoFocus
				aria-label="Position Reference"
				value={controller.ui.editText}
				onChange={(event) => controller.ui.setEditText(event.target.value)}
			>
				<option value="">None</option>
				{points.map((point) => (
					<option key={point.fixture_id} value={point.fixture_id}>
						{positionPointLabel(point)}
					</option>
				))}
			</Select>
			<small>
				Moving or rotating the point carries this{" "}
				{selected && isVisualOnly(selected.definition) ? "object" : "fixture"} with
				it, relative to the point's own origin. Its location and rotation stay where
				it sits when the point rests on its origin.
			</small>
		</label>
	);
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
				Portable logical names only. This desk resolves local folders and
				devices in Setup.
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
				<ErrorAlert as="p" className="patch-policy-warning" role="alert">
					This fixture may remain live while an applicable master is reduced.
				</ErrorAlert>
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
	const {ui} = controller;
	if (!family) return null;
	const query = ui.replacementQuery.trim().toLowerCase();
	const modes = ui.replacingFixture ? controller.data.availableDefinitions.filter(mode =>
		fixtureDefinitionKey(mode) === ui.definitionKey || `${mode.manufacturer} ${mode.name || mode.model} ${mode.mode}`.toLowerCase().includes(query)) : family.modes;
	const target = controller.data.definition;
	const selected = controller.data.selected;
	const targetHeads = target?.profile_snapshot?.modes.find(mode => mode.id === target.mode_id)?.heads ?? [];
	return <>
		{ui.replacingFixture && <label>Find replacement<TextInput aria-label="Find replacement" value={ui.replacementQuery} onChange={event => ui.setReplacementQuery(event.target.value)} /></label>}
		<label>Product / mode
			<Select aria-label="Product / mode" value={ui.definitionKey}
				onChange={event => { ui.setDefinitionKey(event.target.value); ui.setReplacementHeads({}); }}>
				{modes.map(mode => <option value={fixtureDefinitionKey(mode)} key={fixtureDefinitionKey(mode)}>
					{ui.replacingFixture ? `${mode.manufacturer} · ${mode.name || mode.model} · ` : ""}{mode.mode} · {mode.footprint}ch
				</option>)}
			</Select>
		</label>
		{!ui.replacingFixture && <Button onClick={() => {ui.setReplacingFixture(true); ui.setReplacementHeads({});}}>Replace fixture with another product</Button>}
		{ui.replacingFixture && <>
			<p>The fixture number, placement, groups and stored programming stay attached to this fixture. Choose each logical head correspondence explicitly. Unmatched heads keep dormant programming; new heads receive new identities.</p>
			<p>Existing root and copy addresses are retained by split number and checked against the new footprint. New splits start unpatched. Incompatible installed calibration remains stored with its original identity and needs revalidation. Direct colors may only approximate on another model; unsupported attributes remain passive.</p>
			{rootProgrammingCorrespondences(selected?.definition, target).map(row => {
				const choice = ui.replacementHeads[row.key] ?? "";
				const destinations = choice === "__unmapped" ? [] : choice.split(",").filter(Boolean);
				return <fieldset key={row.key}>
					<legend>Existing shared head {row.sourceName} · {row.attribute}</legend>
					<p>Route existing programming to the selected owners. New master-only edits keep their normal meaning.</p>
					<label><input type="checkbox" aria-label={`Leave ${row.sourceName} ${row.attribute} unmatched`} checked={choice === "__unmapped"}
						onChange={event => ui.setReplacementHeads({...ui.replacementHeads, [row.key]: event.target.checked ? "__unmapped" : ""})} /> Leave unmatched — keep dormant programming</label>
					{row.targets.map(target => <label key={target.id}><input type="checkbox"
						aria-label={`Route ${row.sourceName} ${row.attribute} to ${target.name}`} checked={destinations.includes(target.id)}
						onChange={event => ui.setReplacementHeads({...ui.replacementHeads, [row.key]: (event.target.checked
							? [...destinations, target.id] : destinations.filter(id => id !== target.id)).join(",")})} /> {target.name}</label>)}
				</fieldset>;
			})}
			{(selected?.logical_heads ?? []).map(head => <label key={head.fixture_id}>
				Existing head {head.head_index + 1}
				<Select aria-label={`Replacement for head ${head.head_index + 1}`} value={ui.replacementHeads[head.fixture_id] ?? ""}
					onChange={event => ui.setReplacementHeads({...ui.replacementHeads, [head.fixture_id]: event.target.value})}>
					<option value="">Choose correspondence</option>
					<option value="__unmapped">Leave unmatched — keep dormant programming</option>
					{targetHeads.map((head,index) => ({head,index})).filter(({head}) => !head.master_shared).map(({head,index}) => <option key={head.id} value={head.id}>{head.name || "Head"} · {index + 1}</option>)}
				</Select>
			</label>)}
		</>}
	</>;
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
		<ErrorAlert as="p" className="patch-status" role="alert">
			{error}
		</ErrorAlert>
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
	const scenery = SCENERY_AXES.find((entry) => entry.edit === edit);
	if (scenery) return scenery.label;
	if (edit === "crowd_width") return "Crowd width";
	if (edit === "crowd_depth") return "Crowd depth";
	if (edit === "scenery_colour") return "Colour";
	if (edit === "chain") return "Chain";
	if (edit === "model_scale") return "Scale";
	if (edit === "position_reference") return "Position Reference";
	return edit;
}
