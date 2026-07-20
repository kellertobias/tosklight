import type { PatchedFixture } from "../../../api/types";
import { changedPatchFixtureCandidate } from "../../../features/patch/PatchContext";
import { isDmxPatchable } from "../patchUtils";
import type { PatchController, PatchRowMouseEvent } from "./controller";
import { cancelEdit, completeEdit } from "./editSession";
import {
	fixtureDisplayId,
	parseFixtureNumber,
	parseVirtualFixtureNumber,
} from "./fixtureIds";
import { unpatchFixtureChanges } from "./patchModel";

export async function createLayer(
	controller: PatchController,
	value = controller.ui.layerName,
) {
	const name = value.trim();
	if (!name) return;
	const id = crypto.randomUUID();
	if (
		await controller.server.savePatchLayer({
			id,
			name,
			order: controller.data.layers.length,
		})
	) {
		controller.ui.setActiveLayer(id);
		controller.ui.setLayerName("");
		controller.ui.setLayerModal(null);
	}
}

export async function selectLayer(
	controller: PatchController,
	layerId: string,
) {
	const selected = controller.data.selected;
	if (
		selected &&
		(await controller.patch.updateFixture(selected.fixture_id, {
			layer_id: layerId,
		}))
	) {
		controller.ui.setLayerModal(null);
		controller.dispatch({ type: "SET_PATCH_ARMED", value: false });
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

export function requestFixtureDelete(
	controller: PatchController,
	fixture: PatchedFixture,
) {
	controller.ui.setSelectedFixture(fixture.fixture_id);
	controller.ui.setDeleteConfirm(fixture);
	controller.ui.setDeleteArmed(false);
}

export async function deleteFixture(controller: PatchController) {
	const fixture = controller.ui.deleteConfirm;
	if (!fixture) return;
	if (await controller.patch.deleteFixture(fixture.fixture_id)) {
		controller.ui.setDeleteConfirm(null);
		controller.ui.setDeleteArmed(false);
		if (controller.ui.selectedFixture === fixture.fixture_id)
			controller.ui.setSelectedFixture(null);
		cancelEdit(controller);
	}
}

export async function unpatchFixtureFromDeleteConfirm(
	controller: PatchController,
) {
	const fixture = controller.ui.deleteConfirm;
	if (!fixture) return;
	if (
		await controller.patch.updateFixture(
			fixture.fixture_id,
			unpatchFixtureChanges(fixture),
		)
	) {
		controller.ui.setDeleteConfirm(null);
		controller.ui.setDeleteArmed(false);
		cancelEdit(controller);
	}
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
			changedPatchFixtureCandidate(
				fixture,
				unpatchFixtureChanges(fixture),
			),
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
		controller.dispatch({ type: "SET_PATCH_ARMED", value: false });
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
		controller.dispatch({ type: "SET_PATCH_ARMED", value: false });
}

export function selectPatchFixture(
	controller: PatchController,
	fixture: PatchedFixture,
	event: PatchRowMouseEvent,
) {
	const { ui, appState } = controller;
	if (ui.deleteArmed) {
		requestFixtureDelete(controller, fixture);
		return;
	}
	ui.setSelectedFixture(fixture.fixture_id);
	if (appState.patchSetArmed) return;
	const ordered = controller.data.visible.map(
		(candidate) => candidate.fixture_id,
	);
	if (event.shiftKey && ui.selectionAnchor.current) {
		selectFixtureRange(controller, ordered, fixture.fixture_id);
	} else if (event.ctrlKey || event.metaKey) {
		selectFixtureAdditively(controller, fixture);
	} else {
		void controller.selection.actions?.replace({
			resolvedFixtures: [fixture.fixture_id],
		});
	}
	ui.selectionAnchor.current = fixture.fixture_id;
}

function selectFixtureRange(
	controller: PatchController,
	ordered: string[],
	fixtureId: string,
) {
	const anchor = controller.ui.selectionAnchor.current;
	if (!anchor) return;
	const from = ordered.indexOf(anchor);
	const to = ordered.indexOf(fixtureId);
	if (from >= 0 && to >= 0)
		void controller.selection.actions?.replace({
			resolvedFixtures: ordered.slice(
				Math.min(from, to),
				Math.max(from, to) + 1,
			),
		});
}

function selectFixtureAdditively(
	controller: PatchController,
	fixture: PatchedFixture,
) {
	const current = controller.selection.fixtureIds;
	const actions = controller.selection.actions;
	if (!current || !actions) return;
	const members = fixture.logical_heads.length
		? fixture.logical_heads.map((head) => head.fixture_id)
		: [fixture.fixture_id];
	void actions.gesture({
		source: { type: "fixture", fixtureId: fixture.fixture_id },
		resolvedFixtures: members,
		operation: members.every((member) => current.has(member)) ? "remove" : "add",
	});
}
