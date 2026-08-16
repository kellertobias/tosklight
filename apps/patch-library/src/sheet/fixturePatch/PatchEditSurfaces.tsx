import {
	ModalRegistration,
	ModalTitleBar,
	NumberField,
	Select,
	TextInput,
} from "@tosklight/ui";
import { ModalNumberEditor } from "@tosklight/ui/input";
import { parsePatchAddress } from "../fields";
import { fixtureDefinitionKey } from "../fixtureProfileModel";
import { isDmxPatchable } from "../patchUtils";
import { usePatchController } from "./controller";
import { saveEdit, saveSplitEdit } from "./editSave";
import { cancelEdit, requestFixtureEditClose } from "./editSession";
import { FixtureAddressScreen } from "./FixtureAddressScreen";
import { parseFixtureNumber, parseVirtualFixtureNumber } from "./fixtureIds";
import {
	closeMultipatchEdit,
	requestMultipatchEditClose,
	saveMultipatchEdit,
} from "./multipatchActions";
import { definitionSplits, replaceSelectedSplitPatch } from "./patchModel";
import { selectedFixturesInOperatorOrder } from "./selection";

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
	if (
		!edit ||
		controller.ui.editPresentation !== "modal" ||
		!controller.data.selected ||
		edit === "address"
	)
		return null;
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

export function DesktopValueEntryDialog() {
	const controller = usePatchController();
	const edit = controller.ui.edit;
	const selected = controller.data.selected;
	if (
		controller.ui.editPresentation !== "value_entry" ||
		!isDesktopNumericEdit(edit) ||
		!selected
	)
		return null;
	const fixtures = selectedFixturesInOperatorOrder(controller);
	const targets = fixtures.length ? fixtures : [selected];
	const label = desktopNumericEditLabel(edit, controller.ui.editAxis);
	return (
		<ModalNumberEditor
			ariaLabel={label}
			title={`${label} · ${targets.length} fixture${targets.length === 1 ? "" : "s"}`}
			beforeTitle={
				controller.ui.editError ? (
					<output className="patch-status" role="alert">
						{controller.ui.editError}
					</output>
				) : undefined
			}
			value={controller.ui.editText}
			onChange={controller.ui.setEditText}
			onSubmit={(value) =>
				void applyDesktopValueEntry(controller, targets, edit, value ?? "")
			}
			onClose={() => cancelEdit(controller)}
			allowThrough={targets.length > 1}
			unit={desktopNumericEditUnit(edit)}
		/>
	);
}

async function applyDesktopValueEntry(
	controller: ReturnType<typeof usePatchController>,
	fixtures: ReturnType<typeof usePatchController>["data"]["visible"],
	edit: Exclude<ReturnType<typeof usePatchController>["ui"]["edit"], null>,
	value: string,
) {
	controller.ui.setEditError("");
	if (value.trim() === controller.ui.editBaseline.trim()) {
		cancelEdit(controller);
		return;
	}
	const values = spreadInput(value, fixtures.length);
	if (!values) {
		controller.ui.setEditError(
			"Enter one value, or two endpoints separated by THRU for a selection.",
		);
		return;
	}
	for (const [index, fixture] of fixtures.entries()) {
		const raw = values[index];
		let changes: Partial<(typeof fixtures)[number]> | null = null;
		if (edit === "number") {
			if (isDmxPatchable(fixture.definition)) {
				const fixtureNumber = parseFixtureNumber(raw);
				if (fixtureNumber != null)
					changes = {
						fixture_number: fixtureNumber,
						virtual_fixture_number: null,
					};
			} else {
				const virtualNumber = parseVirtualFixtureNumber(raw);
				if (virtualNumber != null)
					changes = {
						fixture_number: null,
						virtual_fixture_number: virtualNumber,
					};
			}
		} else if (edit === "address") {
			const address = parsePatchAddress(raw);
			if (address)
				changes =
					fixture.definition.schema_version >= 2
						? replaceSelectedSplitPatch(
								fixture.definition,
								fixture.split_patches,
								fixture.universe,
								fixture.address,
								definitionSplits(fixture.definition)[0]?.number ?? 1,
								address,
							)
						: address;
		} else
			changes = numericFixtureChanges(
				fixture,
				edit,
				controller.ui.editAxis,
				raw,
			);
		if (!changes) {
			controller.ui.setEditError(
				`“${raw}” is not a valid ${desktopNumericEditLabel(edit, controller.ui.editAxis).toLowerCase()}.`,
			);
			return;
		}
		if (!(await controller.patch.updateFixture(fixture.fixture_id, changes))) {
			controller.ui.setEditError(
				`Could not update ${fixture.name || fixture.fixture_id}.`,
			);
			return;
		}
	}
	cancelEdit(controller);
}

function isDesktopNumericEdit(
	edit: ReturnType<typeof usePatchController>["ui"]["edit"],
): edit is
	| "number"
	| "address"
	| "mib_delay"
	| "location"
	| "rotation"
	| "bracket_angle"
	| "shaper_angle" {
	return (
		edit === "number" ||
		edit === "address" ||
		edit === "mib_delay" ||
		edit === "location" ||
		edit === "rotation" ||
		edit === "bracket_angle" ||
		edit === "shaper_angle"
	);
}

function desktopNumericEditLabel(
	edit: Exclude<ReturnType<typeof usePatchController>["ui"]["edit"], null>,
	axis: ReturnType<typeof usePatchController>["ui"]["editAxis"],
) {
	if (edit === "number") return "Fixture ID";
	if (edit === "address") return "Patch";
	if (edit === "mib_delay") return "MIB Delay";
	if (edit === "bracket_angle") return "Bracket angle";
	if (edit === "shaper_angle") return "Shaper angle";
	if (edit === "location" || edit === "rotation")
		return `${edit === "location" ? "Location" : "Rotation"} ${axis?.toUpperCase() ?? "value"}`;
	return "Value";
}

function desktopNumericEditUnit(
	edit: Exclude<ReturnType<typeof usePatchController>["ui"]["edit"], null>,
) {
	if (edit === "location") return "meter";
	if (
		edit === "rotation" ||
		edit === "bracket_angle" ||
		edit === "shaper_angle"
	)
		return "degree";
	if (edit === "mib_delay") return "second";
	return undefined;
}

function numericFixtureChanges(
	fixture: ReturnType<typeof usePatchController>["data"]["visible"][number],
	edit: Exclude<ReturnType<typeof usePatchController>["ui"]["edit"], null>,
	axis: ReturnType<typeof usePatchController>["ui"]["editAxis"],
	raw: string,
) {
	const value = Number(raw);
	if (!Number.isFinite(value)) return null;
	if (edit === "mib_delay")
		return {
			move_in_black_delay_millis: Math.max(0, Math.round(value * 1000)),
		};
	if (edit === "bracket_angle") return { bracket_angle: value };
	if (edit === "shaper_angle") return { shaper_angle: value };
	if ((edit === "location" || edit === "rotation") && axis)
		return {
			[edit]: {
				...(fixture[edit] ?? { x: 0, y: 0, z: 0 }),
				[axis]: edit === "location" ? Math.round(value * 1000) : value,
			},
		};
	return null;
}

function spreadInput(value: string, count: number): string[] | null {
	const points = value
		.trim()
		.split(/\s+THRU\s+/iu)
		.map((point) => point.trim())
		.filter(Boolean);
	if (points.length === 1)
		return Array.from({ length: count }, () => points[0]);
	if (points.length !== 2 || count < 2) return null;
	const addresses = points.map(parsePatchAddress);
	if (addresses.every((address) => address != null)) {
		const [first, last] = addresses as Array<{
			universe: number;
			address: number;
		}>;
		const start = (first.universe - 1) * 512 + first.address - 1;
		const end = (last.universe - 1) * 512 + last.address - 1;
		return Array.from({ length: count }, (_, index) => {
			const slot = Math.round(start + ((end - start) * index) / (count - 1));
			return `${Math.floor(slot / 512) + 1}.${(slot % 512) + 1}`;
		});
	}
	const virtual = points.map(parseVirtualFixtureNumber);
	if (virtual.every((number) => number != null))
		return interpolate(virtual as number[], count).map(
			(number) => `0.${number}`,
		);
	const fixtureNumbers = points.map(parseFixtureNumber);
	if (fixtureNumbers.every((number) => number != null))
		return interpolate(fixtureNumbers as number[], count).map(String);
	const numeric = points.map(Number);
	if (numeric.every(Number.isFinite))
		return interpolate(numeric, count).map((number) =>
			String(Number(number.toFixed(6))),
		);
	return null;
}

function interpolate([first, last]: number[], count: number) {
	return Array.from(
		{ length: count },
		(_, index) => first + ((last - first) * index) / (count - 1),
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
	if (
		controller.ui.edit !== "address" ||
		controller.ui.editPresentation !== "modal" ||
		controller.ui.pending ||
		!selected
	)
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
	return edit;
}
