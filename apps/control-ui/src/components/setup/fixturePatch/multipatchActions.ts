import type { MultiPatchInstance, SplitPatch } from "../../../api/types";
import { parsePatchAddress } from "../../input/ConsoleFields";
import type { MultiPatchEdit, PatchController } from "./controller";
import {
	definitionSplits,
	effectiveSplitPatches,
	replaceSelectedSplitPatch,
	splitPatchSetError,
} from "./patchModel";

export async function addMultipatch(controller: PatchController) {
	const selected = controller.data.selected;
	if (!selected) return;
	const instance: MultiPatchInstance = {
		id: crypto.randomUUID(),
		name: "multi-patch",
		universe: null,
		address: null,
		split_patches: definitionSplits(selected.definition).map((split) => ({
			split: split.number,
			universe: null,
			address: null,
		})),
		location: { x: 0, y: 0, z: 0 },
		rotation: { x: 0, y: 0, z: 0 },
	};
	await controller.server.updatePatchedFixture(selected.fixture_id, {
		multipatch: [...(selected.multipatch ?? []), instance],
	});
}

export function beginMultipatchEdit(
	controller: PatchController,
	fixture: PatchController["data"]["all"][number],
	instance: MultiPatchInstance,
	kind: NonNullable<MultiPatchEdit>["kind"],
) {
	const { ui } = controller;
	ui.setEditError("");
	ui.setSelectedFixture(fixture.fixture_id);
	ui.setMultipatchEdit({
		fixtureId: fixture.fixture_id,
		instanceId: instance.id,
		kind,
	});
	if (kind === "address") {
		ui.setEditText(
			instance.universe && instance.address
				? `${instance.universe}.${instance.address}`
				: "",
		);
		ui.setEditSplitDrafts(
			Object.fromEntries(
				effectiveSplitPatches(
					fixture.definition,
					instance.split_patches,
					instance.universe,
					instance.address,
				).map((patch) => [
					patch.split,
					patch.universe && patch.address
						? `${patch.universe}.${patch.address}`
						: "",
				]),
			),
		);
	} else ui.setVector(instance[kind]);
}

export async function saveMultipatchEdit(
	controller: PatchController,
	value = controller.ui.editText,
) {
	const edit = controller.ui.multipatchEdit;
	if (!edit) return;
	controller.ui.setEditError("");
	const fixture = controller.data.all.find(
		(item) => item.fixture_id === edit.fixtureId,
	);
	const instance = fixture?.multipatch?.find(
		(item) => item.id === edit.instanceId,
	);
	if (!fixture || !instance) return;
	const changes = multipatchChanges(controller, fixture, instance, value);
	if (!changes) return;
	const multipatch = (fixture.multipatch ?? []).map((item) =>
		item.id === instance.id ? { ...item, ...changes } : item,
	);
	if (
		await controller.server.updatePatchedFixture(fixture.fixture_id, {
			multipatch,
		})
	)
		controller.ui.setMultipatchEdit(null);
}

function multipatchChanges(
	controller: PatchController,
	fixture: PatchController["data"]["all"][number],
	instance: MultiPatchInstance,
	value: string,
): Partial<MultiPatchInstance> | null {
	const edit = controller.ui.multipatchEdit;
	if (!edit) return null;
	if (
		edit.kind === "address" &&
		definitionSplits(fixture.definition).length > 1
	)
		return splitAddressChanges(controller, fixture);
	if (edit.kind === "address")
		return singleAddressChanges(controller, fixture, instance, value);
	return { [edit.kind]: controller.ui.vector };
}

function splitAddressChanges(
	controller: PatchController,
	fixture: PatchController["data"]["all"][number],
) {
	const parsed = definitionSplits(fixture.definition).map((split) => {
		const raw = controller.ui.editSplitDrafts[split.number]?.trim() ?? "";
		return {
			split: split.number,
			raw,
			value: raw ? parsePatchAddress(raw) : null,
		};
	});
	if (parsed.some((item) => item.raw && !item.value)) {
		controller.ui.setEditError(
			"Enter split patches as universe.address, for example 1.101.",
		);
		return null;
	}
	const split_patches: SplitPatch[] = parsed.map((item) => ({
		split: item.split,
		universe: item.value?.universe ?? null,
		address: item.value?.address ?? null,
	}));
	const invalid = splitPatchSetError(fixture.definition, split_patches);
	if (invalid) {
		controller.ui.setEditError(invalid);
		return null;
	}
	const primary =
		split_patches.find((patch) => patch.split === 1) ?? split_patches[0];
	return {
		split_patches,
		universe: primary?.universe ?? null,
		address: primary?.address ?? null,
	};
}

function singleAddressChanges(
	controller: PatchController,
	fixture: PatchController["data"]["all"][number],
	instance: MultiPatchInstance,
	value: string,
) {
	const parsed = parsePatchAddress(value);
	if (fixture.definition.schema_version >= 2) {
		const split = definitionSplits(fixture.definition)[0]?.number ?? 1;
		if (parsed || !value.trim() || value.trim() === "0")
			return replaceSelectedSplitPatch(
				fixture.definition,
				instance.split_patches,
				instance.universe,
				instance.address,
				split,
				parsed,
			);
	} else if (parsed) return parsed;
	else if (!value.trim() || value.trim() === "0")
		return { universe: null, address: null };
	controller.ui.setEditError(
		"Enter the patch as universe.address or clear it to unpatch.",
	);
	return null;
}

export function closeMultipatchEdit(controller: PatchController) {
	controller.ui.setEditError("");
	controller.ui.setMultipatchEdit(null);
}

export function multipatchVectorIsDirty(controller: PatchController) {
	const edit = controller.ui.multipatchEdit;
	if (!edit || edit.kind === "address") return false;
	const fixture = controller.data.all.find(
		(item) => item.fixture_id === edit.fixtureId,
	);
	const instance = fixture?.multipatch?.find(
		(item) => item.id === edit.instanceId,
	);
	return Boolean(
		instance &&
			JSON.stringify(controller.ui.vector) !==
				JSON.stringify(instance[edit.kind]),
	);
}

export function requestMultipatchEditClose(controller: PatchController) {
	if (multipatchVectorIsDirty(controller))
		controller.ui.setEditCloseConfirm("multipatch");
	else closeMultipatchEdit(controller);
}
