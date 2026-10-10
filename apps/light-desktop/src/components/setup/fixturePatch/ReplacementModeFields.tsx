import {
	Button,
	ErrorAlert,
	ModalRegistration,
	ModalTitleBar,
	SelectField,
	TextField,
} from "@tosklight/ui";
import { fixtureDefinitionKey } from "../fixtureProfileModel";
import { usePatchController } from "./controller";
import { saveEdit } from "./editSave";
import {
	ReplacementDecisions,
	replacementDecisionRows,
} from "./ReplacementDecisions";

export function ReplacementEditDialog({
	close,
	pending,
}: {
	close(): void;
	pending: boolean;
}) {
	const controller = usePatchController();
	const rows = replacementDecisionRows(controller);
	const unresolved = controller.ui.replacingFixture
		? rows.filter((row) => row.state === "required" || row.state === "invalid")
				.length
		: 0;
	const definition = controller.data.definition;
	const unavailable = pending
		? "Validating and applying the replacement. Wait for the authoritative result."
		: unresolved
			? `Resolve ${unresolved} correspondence${unresolved === 1 ? "" : "s"} before Set.`
			: controller.ui.replacingFixture &&
					(!controller.ui.replacementRevision ||
						!definition?.profile_snapshot ||
						!definition.mode_id)
				? "Reopen the fixture editor to obtain a current product, mode and show revision."
				: null;
	const error = controller.ui.editError;
	return (
		<ModalRegistration onClose={close}>
			<div className="stacked-modal-layer">
				<section className="nested-modal patch-edit-modal import-workflow">
					<div className="import-workflow__header">
						<ModalTitleBar
							title="Set fixture mode"
							accept={{
								id: "set",
								label: pending ? "Replacing fixture…" : "Set",
								disabled: Boolean(unavailable),
								variant: "primary",
								onPress: () => saveEdit(controller),
							}}
							closeLabel="Cancel fixture mode"
							closeDisabled={pending}
							onClose={close}
						/>
					</div>
					<div className="import-workflow__body">
						{error && <ReplacementError error={error} />}
						<ReplacementModeFields />
					</div>
					<div className="import-workflow__footer">
						{unavailable && <p role="status">{unavailable}</p>}
						<Button disabled={pending} onClick={close}>
							Cancel
						</Button>
						<Button
							variant="primary"
							disabled={Boolean(unavailable)}
							onClick={() => saveEdit(controller)}
						>
							{pending ? "Replacing fixture…" : "Set"}
						</Button>
					</div>
				</section>
			</div>
		</ModalRegistration>
	);
}

function ReplacementError({ error }: { error: string }) {
	const firstLine = error.split("\n")[0];
	return (
		<div className="import-warning-summary">
			<ErrorAlert as="p" role="alert">
				{firstLine}
			</ErrorAlert>
			<p>
				Review the affected correspondence. If the show or product changed,
				reopen this editor and review the choices again.
			</p>
			{error !== firstLine && (
				<details>
					<summary>Full replacement error</summary>
					<pre>{error}</pre>
				</details>
			)}
		</div>
	);
}

export function ReplacementModeFields() {
	const controller = usePatchController();
	const { ui, data } = controller;
	const family = data.selectedModeFamily;
	if (!family) return null;
	const query = ui.replacementQuery.trim().toLowerCase();
	const modes = ui.replacingFixture
		? data.availableDefinitions.filter(
				(mode) =>
					fixtureDefinitionKey(mode) === ui.definitionKey ||
					`${mode.manufacturer} ${mode.name || mode.model} ${mode.mode}`
						.toLowerCase()
						.includes(query),
			)
		: family.modes;
	const pending = Boolean(
		ui.replacingFixture &&
			data.selected &&
			controller.patch.pendingFixtureIds.has(data.selected.fixture_id),
	);
	const target = data.definition;
	return (
		<>
			<div className="import-workflow__summary">
				<h3>
					{ui.replacingFixture
						? "Replace physical fixture"
						: "Choose fixture mode"}
				</h3>
				<p>
					<strong>Source:</strong> {data.selected?.name} ·{" "}
					{data.selected?.definition.manufacturer} ·{" "}
					{data.selected?.definition.mode}
				</p>
				<p>
					<strong>Destination:</strong> {target?.manufacturer} ·{" "}
					{target?.name || target?.model} · {target?.mode} · {target?.footprint}
					ch
				</p>
			</div>
			{ui.replacingFixture && (
				<TextField
					label="Find replacement"
					disabled={pending}
					aria-label="Find replacement"
					value={ui.replacementQuery}
					onChange={(event) => ui.setReplacementQuery(event.target.value)}
				/>
			)}
			<SelectField
				label="Product / mode"
				disabled={pending}
				value={ui.definitionKey}
				onChange={(value) => {
					ui.setDefinitionKey(value);
					ui.setReplacementHeads({});
				}}
				options={modes.map((mode) => ({
					value: fixtureDefinitionKey(mode),
					label: `${ui.replacingFixture ? `${mode.manufacturer} · ${mode.name || mode.model} · ` : ""}${mode.mode} · ${mode.footprint}ch`,
				}))}
			/>
			{!ui.replacingFixture && (
				<Button
					disabled={pending}
					onClick={() => {
						ui.setReplacingFixture(true);
						ui.setReplacementHeads({});
					}}
				>
					Replace fixture with another product
				</Button>
			)}
			{ui.replacingFixture && (
				<>
					<ReplacementDecisions
						rows={replacementDecisionRows(controller)}
						disabled={pending}
						onChoice={(key, value) =>
							ui.setReplacementHeads({ ...ui.replacementHeads, [key]: value })
						}
					/>
					<ReplacementConsequences />
				</>
			)}
		</>
	);
}

function ReplacementConsequences() {
	return (
		<div className="import-limitations">
			<p className="import-warning-summary">
				Review 4 replacement consequences: dormant programming, patch footprint,
				installed calibration and color approximation.
			</p>
			<details>
				<summary>Programming, addresses and calibration details</summary>
				<p>
					The fixture number, root identity, placement, groups and stored
					programming stay attached to this fixture. Correspondences apply to
					existing Preset, Cue, Group and held Programmer sources. Group
					membership and spread order stay unchanged; fresh master-only edits
					keep their normal meaning.
				</p>
				<p>
					Unmatched heads keep dormant programming; new heads receive new
					identities. Unsupported attributes remain passive. Save/reopen and
					inspect emitted DMX after replacement.
				</p>
				<p>
					Existing root and copy addresses are retained by split number and
					checked against the new footprint. New splits start unpatched; removed
					splits stop output. Overlaps or an invalid footprint reject the entire
					replacement.
				</p>
				<p>
					Incompatible installed calibration remains stored with its original
					identity and needs revalidation. Semantic color and aim resolve
					through the new capabilities. A color wheel may approximate a
					requested color. Direct colors retain their original recipe with
					best-effort translation on another model.
				</p>
			</details>
		</div>
	);
}
