import { changedPatchFixtureCandidate } from "../../state/PatchContext";
import type { PatchedFixture } from "../../wire";
import { isDmxPatchable } from "../patchUtils";
import type { PatchController, PatchRowMouseEvent } from "./controller";
import { cancelEdit, completeEdit } from "./editSession";
import {
	fixtureDisplayId,
	parseFixtureNumber,
	parseVirtualFixtureNumber,
} from "./fixtureIds";
import { unpatchFixtureChanges } from "./patchModel";
import {
	fixtureSelectionIds,
	orderedFixtureSelectionIds,
	editTargets,
	toggledFixtureSelection,
} from "./selection";

export async function createLayer(
	controller: PatchController,
	value = controller.ui.layerName,
) {
	const name = value.trim();
	if (!name) return;
	const id = crypto.randomUUID();
	if (
		await controller.library?.savePatchLayer({
			id,
			name,
			order: controller.data.layers.length,
			locked: false,
			visible2d: true,
			visible3d: true,
		})
	) {
		controller.ui.setActiveLayer(id);
		controller.ui.setLayerName("");
		controller.ui.setLayerModal(null);
	}
}

export async function toggleLayerVisibility(
	controller: PatchController,
	layerId: string,
	surface: "2d" | "3d",
) {
	const layer = controller.data.layers.find(
		(candidate) => candidate.id === layerId,
	);
	if (!layer) return;
	const key = surface === "2d" ? "visible2d" : "visible3d";
	await controller.library?.savePatchLayer({
		...layer,
		[key]: !(layer[key] ?? true),
	});
}

export async function toggleLayerLock(
	controller: PatchController,
	layerId: string,
) {
	const layer = controller.data.layers.find(
		(candidate) => candidate.id === layerId,
	);
	if (!layer) return;
	const locked = !layer.locked;
	if (!(await controller.library?.savePatchLayer({ ...layer, locked }))) return;
	if (
		locked &&
		controller.data.selected &&
		(controller.data.selected.layer_id || "default") === layerId
	)
		controller.ui.setSelectedFixture(null);
}

export async function selectLayer(
	controller: PatchController,
	layerId: string,
) {
	const selected = controller.data.selected;
	if (!selected) return;
	// On a desktop sheet the layer goes to the whole selection, as one patch change.
	const targets = controller.host.desktopEditing
		? editTargets(controller, selected)
		: [selected];
	const moving = targets.filter(
		(fixture) => (fixture.layer_id || "default") !== layerId,
	);
	const applied =
		moving.length === 0 ||
		(moving.length === 1
			? await controller.patch.updateFixture(moving[0].fixture_id, {
					layer_id: layerId,
				})
			: Boolean(
					await controller.patch.patchFixtures(
						moving.map((fixture) =>
							changedPatchFixtureCandidate(fixture, { layer_id: layerId }),
						),
					),
				));
	if (applied) {
		controller.ui.setLayerModal(null);
		controller.host.setEditArmed(false);
	}
}

export async function unpatchCurrentFixture(controller: PatchController) {
	const selected = controller.data.selected;
	if (!selected) return;
	if (
		await controller.patch.updateFixture(
			selected.fixture_id,
			unpatchFixtureChanges(selected),
		)
	)
		cancelEdit(controller);
}

/**
 * Asks whether to delete or unpatch `fixture` or, when it belongs to a selection of several on a
 * desktop sheet, every selected fixture. A selection never sweeps in fixtures on a locked layer.
 */
export function requestFixtureDelete(
	controller: PatchController,
	fixture: PatchedFixture,
) {
	const targets = controller.host.desktopEditing
		? editTargets(controller, fixture).filter(
				(target) =>
					target.fixture_id === fixture.fixture_id ||
					!onLockedLayer(controller, target),
			)
		: [fixture];
	controller.ui.setSelectedFixture(fixture.fixture_id);
	controller.ui.setDeleteConfirm(targets);
	controller.ui.setDeleteArmed(false);
}

function onLockedLayer(controller: PatchController, fixture: PatchedFixture) {
	return Boolean(
		controller.data.layers.find(
			(layer) => layer.id === (fixture.layer_id || "default"),
		)?.locked,
	);
}

/** Deletes every fixture the confirmation names, as one patch change. */
export async function deleteFixture(controller: PatchController) {
	const targets = controller.ui.deleteConfirm;
	if (!targets?.length) return;
	const deleted =
		targets.length === 1
			? await controller.patch.deleteFixture(targets[0].fixture_id)
			: await controller.patch.deleteFixtures(
					targets.map((fixture) => fixture.fixture_id),
				);
	if (!deleted) return;
	controller.ui.setDeleteConfirm(null);
	controller.ui.setDeleteArmed(false);
	if (
		targets.some(
			(fixture) => fixture.fixture_id === controller.ui.selectedFixture,
		)
	)
		controller.ui.setSelectedFixture(null);
	// A deleted selection would otherwise stay selected with nothing behind it.
	if (targets.length > 1)
		void controller.selection.replace({ resolvedFixtures: [] });
	cancelEdit(controller);
}

/** Clears the DMX addresses of every fixture the confirmation names, as one patch change. */
export async function unpatchFixtureFromDeleteConfirm(
	controller: PatchController,
) {
	const targets = controller.ui.deleteConfirm;
	if (!targets?.length) return;
	const unpatched =
		targets.length === 1
			? await controller.patch.updateFixture(
					targets[0].fixture_id,
					unpatchFixtureChanges(targets[0]),
				)
			: Boolean(
					await controller.patch.patchFixtures(
						targets.map((fixture) =>
							changedPatchFixtureCandidate(
								fixture,
								unpatchFixtureChanges(fixture),
							),
						),
					),
				);
	if (!unpatched) return;
	controller.ui.setDeleteConfirm(null);
	controller.ui.setDeleteArmed(false);
	cancelEdit(controller);
}

export async function unpatchConflictsAndApply(controller: PatchController) {
	const { selected } = controller.data;
	const { pending, blockedBy } = controller.ui;
	if (
		!selected ||
		!pending ||
		!window.confirm("Unpatch the conflicting fixtures and apply this change?")
	)
		return;
	const candidates = [
		...blockedBy.map((fixture) =>
			changedPatchFixtureCandidate(fixture, unpatchFixtureChanges(fixture)),
		),
		changedPatchFixtureCandidate(selected, pending),
	];
	if (!(await controller.patch.patchFixtures(candidates))) {
		controller.ui.setEditError(
			"The conflicting fixtures could not be unpatched. No patch changes were applied.",
		);
		return;
	}
	completeEdit(controller);
}

export async function setFixtureNumber(
	controller: PatchController,
	fixture: PatchedFixture,
) {
	const visualOnly = !isDmxPatchable(fixture.definition);
	const value = window.prompt("Fixture ID", String(fixtureDisplayId(fixture)));
	if (value == null) return;
	if (visualOnly) {
		await setVirtualFixtureNumber(controller, fixture, value);
		return;
	}
	const number = parseFixtureNumber(value);
	if (number == null) {
		window.alert("Fixture IDs must be positive whole numbers.");
		return;
	}
	if (
		controller.data.all.some(
			(candidate) =>
				candidate.fixture_id !== fixture.fixture_id &&
				candidate.fixture_number === number,
		)
	) {
		window.alert(`Fixture ID ${number} is already in use.`);
		return;
	}
	if (
		await controller.patch.updateFixture(fixture.fixture_id, {
			fixture_number: number,
			virtual_fixture_number: null,
		})
	)
		controller.host.setEditArmed(false);
}

async function setVirtualFixtureNumber(
	controller: PatchController,
	fixture: PatchedFixture,
	value: string,
) {
	const number = parseVirtualFixtureNumber(value);
	if (number == null) {
		window.alert("Visual fixture IDs must start at 0.1.");
		return;
	}
	if (
		controller.data.all.some(
			(candidate) =>
				candidate.fixture_id !== fixture.fixture_id &&
				candidate.virtual_fixture_number === number,
		)
	) {
		window.alert(`Fixture ID 0.${number} is already in use.`);
		return;
	}
	if (
		await controller.patch.updateFixture(fixture.fixture_id, {
			fixture_number: null,
			virtual_fixture_number: number,
		})
	)
		controller.host.setEditArmed(false);
}

export function selectPatchFixture(
	controller: PatchController,
	fixture: PatchedFixture,
	event: PatchRowMouseEvent,
) {
	const { ui, editArmed } = controller;
	if (
		controller.data.layers.find(
			(layer) => layer.id === (fixture.layer_id || "default"),
		)?.locked
	)
		return;
	if (ui.deleteArmed) {
		requestFixtureDelete(controller, fixture);
		return;
	}
	ui.setSelectedFixture(fixture.fixture_id);
	if (editArmed && !controller.host.desktopEditing) return;
	const ordered = controller.data.visible;
	if (event.shiftKey && ui.selectionAnchor.current) {
		selectFixtureRange(controller, ordered, fixture.fixture_id);
	} else if (event.ctrlKey || event.metaKey) {
		selectFixtureAdditively(controller, fixture);
	} else {
		void controller.selection.replace({
			resolvedFixtures: fixtureSelectionIds(fixture),
		});
	}
	ui.selectionAnchor.current = fixture.fixture_id;
}

/**
 * A right-click on a fixture outside the selection makes that fixture the selection, so the editor
 * it opens edits that fixture rather than the ones selected before. Inside the selection it keeps
 * the selection, and the editor edits all of them.
 */
export function selectContextFixture(
	controller: PatchController,
	fixture: PatchedFixture,
) {
	if (!controller.editArmed) return;
	const selected = controller.selection.fixtureIds;
	if (fixtureSelectionIds(fixture).some((id) => selected?.has(id))) return;
	controller.ui.setSelectedFixture(fixture.fixture_id);
	void controller.selection.replace({
		resolvedFixtures: fixtureSelectionIds(fixture),
	});
	controller.ui.selectionAnchor.current = fixture.fixture_id;
}

export function selectFixtureRange(
	controller: PatchController,
	ordered: readonly PatchedFixture[],
	fixtureId: string,
) {
	const anchor = controller.ui.selectionAnchor.current;
	if (!anchor) return;
	const from = ordered.findIndex((fixture) => fixture.fixture_id === anchor);
	const to = ordered.findIndex((fixture) => fixture.fixture_id === fixtureId);
	if (from >= 0 && to >= 0)
		void controller.selection.replace({
			resolvedFixtures: orderedFixtureSelectionIds(
				ordered.slice(Math.min(from, to), Math.max(from, to) + 1),
			),
		});
}

function selectFixtureAdditively(
	controller: PatchController,
	fixture: PatchedFixture,
) {
	const current = controller.selection.orderedFixtureIds;
	if (!current) return;
	controller.selection.replace({
		resolvedFixtures: toggledFixtureSelection(current, fixture),
	});
}
