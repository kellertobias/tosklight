import type { PatchedFixture } from "../../wire";
import { fixtureDefinitionKey } from "../fixtureProfileModel";
import { fixtureRanges, groupFixtureFamilies } from "../patchUtils";
import type { EditKind, PatchController, VectorAxis } from "./controller";
import {
	definitionSplits,
	effectiveSplitPatches,
	splitPatchSetError,
} from "./patchModel";
import { fixtureSelectionIds } from "./selection";

export function armEdit(
	controller: PatchController,
	fixture: PatchedFixture,
	kind: Exclude<EditKind, null>,
	axis?: VectorAxis,
) {
	const { ui, editArmed } = controller;
	if (!editArmed) return;
	ui.setEditError("");
	ui.setSelectedFixture(fixture.fixture_id);
	if (kind === "number") ui.setEditText(String(fixtureDisplayId(fixture)));
	else if (kind === "name")
		ui.setEditText(fixture.name || fixture.definition.name);
	else if (kind === "address") {
		ui.setEditText(
			fixture.universe && fixture.address
				? `${fixture.universe}.${fixture.address}`
				: "",
		);
		ui.setEditSplitDrafts(splitDraftValues(fixture));
	} else if (kind === "mib")
		ui.setEditText(String(fixture.move_in_black_enabled ?? true));
	else if (kind === "mib_delay")
		ui.setEditText(String((fixture.move_in_black_delay_millis ?? 0) / 1000));
	else if (kind === "group_masters")
		ui.setEditText(String(fixture.group_masters_enabled ?? true));
	else if (kind === "grand_master")
		ui.setEditText(String(fixture.grand_master_enabled ?? true));
	else if (kind === "invert_pan")
		ui.setEditText(String(fixture.invert_pan ?? false));
	else if (kind === "invert_tilt")
		ui.setEditText(String(fixture.invert_tilt ?? false));
	else if (kind === "bracket_angle")
		ui.setEditText(String(fixture.bracket_angle ?? 0));
	// An empty field is a fixture with no shaper or barn door fitted, which is most of them.
	else if (kind === "shaper_angle")
		ui.setEditText(
			fixture.shaper_angle === undefined || fixture.shaper_angle === null
				? ""
				: String(fixture.shaper_angle),
		);
	else if (kind === "location" || kind === "rotation")
		ui.setVector(fixture[kind] ?? { x: 0, y: 0, z: 0 });
	else if (kind === "mode") selectFixtureFamily(controller, fixture);
	ui.setEditAxis(
		kind === "location" || kind === "rotation" ? (axis ?? null) : null,
	);
	ui.setEdit(kind);
}

export function selectSplitAddress(
	controller: PatchController,
	fixture: PatchedFixture,
	split: number,
) {
	const { ui, editArmed, selection } = controller;
	ui.setSelectedFixture(fixture.fixture_id);
	if (!editArmed)
		void selection.replace({
			resolvedFixtures: fixtureSelectionIds(fixture),
		});
	ui.setEditingSplit(split);
	ui.setEditError("");
	ui.setEditSplitDrafts(splitDraftValues(fixture));
	if (editArmed) ui.setEdit("address");
}

export async function finishEdit(
	controller: PatchController,
	changes: Partial<PatchedFixture>,
) {
	const selected = controller.data.selected;
	if (!selected) return false;
	if (!(await controller.patch.updateFixture(selected.fixture_id, changes)))
		return false;
	completeEdit(controller);
	return true;
}

export function completeEdit(controller: PatchController) {
	controller.ui.setEdit(null);
	controller.ui.setEditingSplit(null);
	controller.ui.setPending(null);
	controller.ui.setBlockedBy([]);
	controller.host.setEditArmed(false);
}

export async function applyEdit(
	controller: PatchController,
	changes: Partial<PatchedFixture>,
) {
	const { selected, all } = controller.data;
	const { ui } = controller;
	if (!selected) return;
	ui.setEditError("");
	if (!changesPhysicalPatch(changes)) {
		await finishEdit(controller, changes);
		return;
	}
	const candidate = { ...selected, ...changes };
	const invalid = validatePatchOwners(candidate);
	if (invalid) {
		ui.setEditError(invalid);
		return;
	}
	const ranges = fixtureRanges(candidate);
	const overlap = findInternalOverlap(ranges);
	if (overlap) {
		ui.setEditError(
			`The fixture's split and multi-patch ranges overlap at universe ${overlap}.`,
		);
		return;
	}
	const found = all.filter(
		(fixture) =>
			fixture.fixture_id !== selected.fixture_id &&
			ranges.some((range) =>
				fixtureRanges(fixture).some(
					(other) =>
						other.universe === range.universe &&
						other.start <= range.end &&
						other.end >= range.start,
				),
			),
	);
	if (found.length) {
		ui.setPending(changes);
		ui.setBlockedBy(found);
		return;
	}
	await finishEdit(controller, changes);
}

export function cancelEdit(controller: PatchController) {
	controller.ui.setEdit(null);
	controller.ui.setEditingSplit(null);
	controller.ui.setEditError("");
	controller.ui.setPending(null);
	controller.ui.setBlockedBy([]);
	controller.host.setEditArmed(false);
}

export function fixtureVectorIsDirty(controller: PatchController) {
	const { selected } = controller.data;
	const { edit, vector, editAxis } = controller.ui;
	if (!selected || (edit !== "location" && edit !== "rotation")) return false;
	const stored = selected[edit] ?? { x: 0, y: 0, z: 0 };
	if (editAxis) return vector[editAxis] !== stored[editAxis];
	return JSON.stringify(vector) !== JSON.stringify(stored);
}

export function requestFixtureEditClose(controller: PatchController) {
	if (fixtureVectorIsDirty(controller))
		controller.ui.setEditCloseConfirm("fixture");
	else cancelEdit(controller);
}

function selectFixtureFamily(
	controller: PatchController,
	fixture: PatchedFixture,
) {
	const family = groupFixtureFamilies(
		controller.data.availableDefinitions,
	).find(
		(item) =>
			item.manufacturer === fixture.definition.manufacturer &&
			item.name === (fixture.definition.name || fixture.definition.model),
	);
	if (!family) return;
	controller.ui.setFamilyKey(family.key);
	controller.ui.setDefinitionKey(fixtureDefinitionKey(fixture.definition));
}

function splitDraftValues(fixture: PatchedFixture) {
	return Object.fromEntries(
		effectiveSplitPatches(
			fixture.definition,
			fixture.split_patches,
			fixture.universe,
			fixture.address,
		).map((patch) => [
			patch.split,
			patch.universe && patch.address
				? `${patch.universe}.${patch.address}`
				: "",
		]),
	);
}

function changesPhysicalPatch(changes: Partial<PatchedFixture>) {
	return (
		"definition" in changes ||
		"universe" in changes ||
		"address" in changes ||
		"split_patches" in changes ||
		"multipatch" in changes
	);
}

function validatePatchOwners(candidate: PatchedFixture) {
	const owners = [
		{
			split_patches: candidate.split_patches,
			universe: candidate.universe,
			address: candidate.address,
		},
		...(candidate.multipatch ?? []),
	];
	for (const owner of owners) {
		const patches = effectiveSplitPatches(
			candidate.definition,
			owner.split_patches,
			owner.universe,
			owner.address,
		);
		const invalid = splitPatchSetError(candidate.definition, patches);
		if (invalid) return invalid;
	}
	return null;
}

function findInternalOverlap(ranges: ReturnType<typeof fixtureRanges>) {
	for (let index = 0; index < ranges.length; index++)
		for (let other = index + 1; other < ranges.length; other++) {
			const left = ranges[index];
			const right = ranges[other];
			if (
				left.universe === right.universe &&
				left.start <= right.end &&
				right.start <= left.end
			)
				return left.universe;
		}
	return null;
}

function fixtureDisplayId(
	fixture: Pick<PatchedFixture, "fixture_number" | "virtual_fixture_number">,
) {
	return fixture.virtual_fixture_number != null
		? `0.${fixture.virtual_fixture_number}`
		: (fixture.fixture_number ?? "—");
}

export function ensureSelectedSplitEdit(controller: PatchController) {
	const { selected } = controller.data;
	const { editingSplit } = controller.ui;
	if (
		controller.editArmed &&
		selected &&
		editingSplit != null &&
		definitionSplits(selected.definition).length > 1
	)
		controller.ui.setEdit("address");
}
