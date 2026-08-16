import type { PatchedFixture } from "../../wire";
import { fixtureDefinitionKey } from "../fixtureProfileModel";
import { fixtureRanges, groupFixtureFamilies } from "../patchUtils";
import type {
	EditKind,
	EditPresentation,
	PatchController,
	VectorAxis,
} from "./controller";
import {
	definitionSplits,
	effectiveSplitPatches,
	splitPatchSetError,
} from "./patchModel";
import {
	fixtureSelectionIds,
	selectedFixturesInOperatorOrder,
} from "./selection";

export function armEdit(
	controller: PatchController,
	fixture: PatchedFixture,
	kind: Exclude<EditKind, null>,
	axis?: VectorAxis,
	presentation: EditPresentation = "modal",
) {
	const { ui, editArmed } = controller;
	if (!editArmed) return;
	let baseline = "";
	const setText = (value: string) => {
		baseline = value;
		ui.setEditText(value);
	};
	ui.setEditError("");
	ui.setEditPresentation(presentation);
	ui.setSelectedFixture(fixture.fixture_id);
	if (kind === "number") {
		setText(String(fixtureDisplayId(fixture)));
	} else if (kind === "name") {
		setText(fixture.name || fixture.definition.name);
	} else if (kind === "address") {
		setText(
			fixture.universe && fixture.address
				? `${fixture.universe}.${fixture.address}`
				: "",
		);
		ui.setEditSplitDrafts(splitDraftValues(fixture));
	} else if (kind === "mib")
		setText(String(fixture.move_in_black_enabled ?? true));
	else if (kind === "mib_delay")
		setText(String((fixture.move_in_black_delay_millis ?? 0) / 1000));
	else if (kind === "group_masters")
		setText(String(fixture.group_masters_enabled ?? true));
	else if (kind === "grand_master")
		setText(String(fixture.grand_master_enabled ?? true));
	else if (kind === "invert_pan") setText(String(fixture.invert_pan ?? false));
	else if (kind === "invert_tilt")
		setText(String(fixture.invert_tilt ?? false));
	else if (kind === "bracket_angle")
		setText(String(fixture.bracket_angle ?? 0));
	// An empty field is a fixture with no shaper or barn door fitted, which is most of them.
	else if (kind === "shaper_angle")
		setText(
			fixture.shaper_angle === undefined || fixture.shaper_angle === null
				? ""
				: String(fixture.shaper_angle),
		);
	else if (kind === "location" || kind === "rotation") {
		ui.setVector(fixture[kind] ?? { x: 0, y: 0, z: 0 });
		if (axis) {
			const stored = fixture[kind]?.[axis] ?? 0;
			setText(String(kind === "location" ? stored / 1000 : stored));
		}
	} else if (kind === "mode") selectFixtureFamily(controller, fixture);
	if (presentation === "value_entry") {
		const selected = selectedFixturesInOperatorOrder(controller);
		if (selected.length > 1) {
			const first = numericEditValue(selected[0], kind, axis);
			const last = numericEditValue(selected[selected.length - 1], kind, axis);
			if (first != null && last != null)
				setText(first === last ? first : `${first} THRU ${last}`);
		}
	}
	ui.setEditBaseline(baseline);
	ui.setEditAxis(
		kind === "location" || kind === "rotation" ? (axis ?? null) : null,
	);
	ui.setEdit(kind);
}

function numericEditValue(
	fixture: PatchedFixture,
	kind: Exclude<EditKind, null>,
	axis?: VectorAxis,
): string | null {
	if (kind === "number") return String(fixtureDisplayId(fixture));
	if (kind === "address")
		return fixture.universe != null && fixture.address != null
			? `${fixture.universe}.${fixture.address}`
			: null;
	if (kind === "mib_delay")
		return String((fixture.move_in_black_delay_millis ?? 0) / 1000);
	if (kind === "bracket_angle") return String(fixture.bracket_angle ?? 0);
	if (kind === "shaper_angle")
		return fixture.shaper_angle == null ? null : String(fixture.shaper_angle);
	if ((kind === "location" || kind === "rotation") && axis) {
		const stored = fixture[kind]?.[axis] ?? 0;
		return String(kind === "location" ? stored / 1000 : stored);
	}
	return null;
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
	if (
		controller.host.desktopEditing &&
		ui.editPresentation === "modal" &&
		!changesPhysicalPatch(changes)
	) {
		const selectedIds = controller.selection.fixtureIds;
		const targets = selectedIds
			? all.filter((fixture) =>
					fixtureSelectionIds(fixture).some((id) => selectedIds.has(id)),
				)
			: [];
		if (targets.length > 1) {
			for (const fixture of targets)
				if (
					!(await controller.patch.updateFixture(fixture.fixture_id, changes))
				)
					return;
			completeEdit(controller);
			return;
		}
	}
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
	controller.ui.setEditPresentation("modal");
	controller.host.setEditArmed(false);
}

export function fixtureEditIsDirty(controller: PatchController) {
	if (fixtureVectorIsDirty(controller)) return true;
	if (fixtureAddressIsDirty(controller)) return true;
	if (fixtureModeIsDirty(controller)) return true;
	const { edit, editText, editBaseline } = controller.ui;
	return Boolean(edit) && editText !== editBaseline;
}

function fixtureAddressIsDirty(controller: PatchController) {
	const { selected } = controller.data;
	if (!selected || controller.ui.edit !== "address") return false;
	return (
		JSON.stringify(controller.ui.editSplitDrafts) !==
		JSON.stringify(splitDraftValues(selected))
	);
}

function fixtureModeIsDirty(controller: PatchController) {
	const { selected } = controller.data;
	return Boolean(
		selected &&
			controller.ui.edit === "mode" &&
			controller.ui.definitionKey !== fixtureDefinitionKey(selected.definition),
	);
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
	if (fixtureEditIsDirty(controller))
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
