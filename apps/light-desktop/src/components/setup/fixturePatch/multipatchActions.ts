import type { MultiPatchInstance, SplitPatch } from "../../../api/types";
import { parsePatchAddress } from "../../input/ConsoleFields";
import {
	type CombinedPolicyChoice,
	combinedPolicyChoice,
	combinedPolicyValues,
} from "./combinedPolicy";
import type {
	MultiPatchEdit,
	PatchController,
	PatchRowMouseEvent,
} from "./controller";
import {
	definitionSplits,
	effectiveSplitPatches,
	fixturePolicyApplicability,
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
	await controller.patch.updateFixture(selected.fixture_id, {
		multipatch: [...(selected.multipatch ?? []), instance],
	});
}

export const PRIMARY_PHYSICAL_PATCH = "primary";

export function selectPhysicalPatchRow(
	controller: PatchController,
	fixture: PatchController["data"]["all"][number],
	physicalId: string,
	event: PatchRowMouseEvent,
) {
	const ordered = [
		PRIMARY_PHYSICAL_PATCH,
		...(fixture.multipatch ?? []).map((instance) => instance.id),
	];
	const sameFixture =
		controller.ui.physicalSelectionFixture === fixture.fixture_id;
	const anchor = controller.ui.physicalSelectionAnchor.current;
	if (event.shiftKey && sameFixture && anchor) {
		const from = ordered.indexOf(anchor);
		const to = ordered.indexOf(physicalId);
		if (from >= 0 && to >= 0)
			controller.ui.setPhysicalSelectionIds(
				ordered.slice(Math.min(from, to), Math.max(from, to) + 1),
			);
	} else controller.ui.setPhysicalSelectionIds([physicalId]);
	controller.ui.setPhysicalSelectionFixture(fixture.fixture_id);
	controller.ui.physicalSelectionAnchor.current = physicalId;
}

export function beginMultipatchEdit(
	controller: PatchController,
	fixture: PatchController["data"]["all"][number],
	instance: MultiPatchInstance,
	kind: NonNullable<MultiPatchEdit>["kind"],
	axis?: NonNullable<MultiPatchEdit>["axis"],
) {
	const { ui } = controller;
	if (kind === "pan_tilt" && !controller.appState.patchSetArmed) return;
	ui.setEditError("");
	ui.setSelectedFixture(fixture.fixture_id);
	ui.setMultipatchEdit({
		fixtureId: fixture.fixture_id,
		instanceId: instance.id,
		kind,
		axis,
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
	} else if (kind === "pan_tilt")
		ui.setEditText(panTiltChoice(fixture, instance));
	else if (kind === "bracket_angle")
		ui.setEditText(String(instance.bracket_angle ?? 0));
	// An empty field is an instance with no shaper or barn door fitted.
	else if (kind === "shaper_angle")
		ui.setEditText(
			instance.shaper_angle === undefined || instance.shaper_angle === null
				? ""
				: String(instance.shaper_angle),
		);
	else {
		ui.setVector(instance[kind]);
		if (axis)
			ui.setEditText(
				String(
					kind === "location"
						? instance[kind][axis] / 1_000
						: instance[kind][axis],
				),
			);
	}
}

function panTiltChoice(
	fixture: PatchController["data"]["all"][number],
	instance: MultiPatchInstance,
) {
	const applicable = fixturePolicyApplicability(fixture.definition);
	return combinedPolicyChoice(
		applicable.pan && (instance.invert_pan ?? false),
		applicable.tilt && (instance.invert_tilt ?? false),
	);
}

export function beginMultipatchVectorEditFromContextMenu(
	controller: PatchController,
	fixture: PatchController["data"]["all"][number],
	instance: MultiPatchInstance,
	kind: "location" | "rotation",
	axis: "x" | "y" | "z",
) {
	controller.dispatch({ type: "SET_PATCH_ARMED", value: true });
	const selected =
		controller.ui.physicalSelectionFixture === fixture.fixture_id &&
		controller.ui.physicalSelectionIds.includes(instance.id)
			? controller.ui.physicalSelectionIds
			: [instance.id];
	controller.ui.setEditError("");
	controller.ui.setSelectedFixture(fixture.fixture_id);
	controller.ui.setEditText(
		String(
			kind === "location" ? instance[kind][axis] / 1_000 : instance[kind][axis],
		),
	);
	controller.ui.setMultipatchEdit({
		fixtureId: fixture.fixture_id,
		instanceId: instance.id,
		kind,
		axis,
		physicalTargets: selected,
	});
}

export async function saveMultipatchVectorInput(
	controller: PatchController,
	value: string,
) {
	const edit = controller.ui.multipatchEdit;
	if (
		!edit ||
		(edit.kind !== "location" && edit.kind !== "rotation") ||
		!edit.axis
	)
		return;
	const fixture = controller.data.all.find(
		(candidate) => candidate.fixture_id === edit.fixtureId,
	);
	if (!fixture) return;
	const kind = edit.kind;
	const axis = edit.axis;
	const targets = edit.physicalTargets?.length
		? edit.physicalTargets
		: [edit.instanceId];
	const endpoints = value
		.split(/\s+THRU\s+/i)
		.map((entry) => Number(entry.trim()));
	if (
		!endpoints.length ||
		endpoints.length > 2 ||
		endpoints.some((entry) => !Number.isFinite(entry))
	) {
		controller.ui.setEditError(
			"Enter a number or two numeric values separated by THRU.",
		);
		return;
	}
	const values = Array.from({ length: targets.length }, (_, index) => {
		const point =
			endpoints.length === 1 || targets.length === 1
				? endpoints[0]
				: endpoints[0] +
					((endpoints[1] - endpoints[0]) * index) / (targets.length - 1);
		return kind === "location" ? Math.round(point * 1_000) : point;
	});
	let primary = fixture[kind] ?? { x: 0, y: 0, z: 0 };
	const byTarget = new Map(
		targets.map((target, index) => [target, values[index]]),
	);
	if (byTarget.has(PRIMARY_PHYSICAL_PATCH))
		primary = { ...primary, [axis]: byTarget.get(PRIMARY_PHYSICAL_PATCH) };
	const multipatch = (fixture.multipatch ?? []).map((instance) =>
		byTarget.has(instance.id)
			? {
					...instance,
					[kind]: {
						...instance[kind],
						[axis]: byTarget.get(instance.id),
					},
				}
			: instance,
	);
	const changes = byTarget.has(PRIMARY_PHYSICAL_PATCH)
		? { [kind]: primary, multipatch }
		: { multipatch };
	if (await controller.patch.updateFixture(fixture.fixture_id, changes))
		controller.ui.setMultipatchEdit(null);
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
	if (edit.kind === "pan_tilt") {
		if (
			await controller.patch.updateFixture(fixture.fixture_id, { multipatch })
		) {
			controller.ui.setMultipatchEdit(null);
			controller.dispatch({ type: "SET_PATCH_ARMED", value: false });
		}
		return;
	}
	if (
		await controller.patch.updateFixture(fixture.fixture_id, {
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
	if (edit.kind === "pan_tilt") {
		const choice = combinedPolicyValues(value as CombinedPolicyChoice);
		const applicable = fixturePolicyApplicability(fixture.definition);
		return {
			invert_pan: applicable.pan
				? choice.first
				: (instance.invert_pan ?? false),
			invert_tilt: applicable.tilt
				? choice.second
				: (instance.invert_tilt ?? false),
		};
	}
	if (
		edit.kind === "address" &&
		definitionSplits(fixture.definition).length > 1
	)
		return splitAddressChanges(controller, fixture);
	if (edit.kind === "address")
		return singleAddressChanges(controller, fixture, instance, value);
	if (edit.kind === "bracket_angle") {
		const degrees = Number(value);
		return Number.isFinite(degrees) ? { bracket_angle: degrees } : null;
	}
	// Clearing the field takes the module off this instance again.
	if (edit.kind === "shaper_angle") {
		const trimmed = value.trim();
		if (!trimmed) return { shaper_angle: null };
		const degrees = Number(trimmed);
		return Number.isFinite(degrees) ? { shaper_angle: degrees } : null;
	}
	if (edit.kind !== "location" && edit.kind !== "rotation") return null;
	// A single-axis edit recomposes over the instance's current siblings so it can
	// never resubmit a stale value for an axis it did not touch.
	return {
		[edit.kind]: edit.axis
			? { ...instance[edit.kind], [edit.axis]: controller.ui.vector[edit.axis] }
			: controller.ui.vector,
	};
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
	if (!edit || edit.kind === "address" || edit.kind === "pan_tilt")
		return false;
	const fixture = controller.data.all.find(
		(item) => item.fixture_id === edit.fixtureId,
	);
	const instance = fixture?.multipatch?.find(
		(item) => item.id === edit.instanceId,
	);
	if (!instance || (edit.kind !== "location" && edit.kind !== "rotation"))
		return false;
	if (edit.axis)
		return controller.ui.vector[edit.axis] !== instance[edit.kind][edit.axis];
	return (
		JSON.stringify(controller.ui.vector) !== JSON.stringify(instance[edit.kind])
	);
}

export function requestMultipatchEditClose(controller: PatchController) {
	if (multipatchVectorIsDirty(controller))
		controller.ui.setEditCloseConfirm("multipatch");
	else closeMultipatchEdit(controller);
}
