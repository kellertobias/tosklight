import type { SplitPatch } from "../../../api/types";
import { parsePatchAddress } from "../../input/ConsoleFields";
import { compatibleHighlightOverrides, isDmxPatchable } from "../patchUtils";
import type { PatchController } from "./controller";
import { applyEdit, completeEdit } from "./editSession";
import { parseFixtureNumber, parseVirtualFixtureNumber } from "./fixtureIds";
import {
	definitionSplits,
	reconcileModePatchChanges,
	replaceSelectedSplitPatch,
} from "./patchModel";

export function saveEdit(
	controller: PatchController,
	value = controller.ui.editText,
) {
	const { selected, all, definition } = controller.data;
	const { edit, vector, editAxis } = controller.ui;
	if (!selected) return;
	if (edit === "number") saveFixtureNumber(controller, value);
	if (edit === "name")
		void applyEdit(controller, { name: value.trim() || selected.name });
	if (edit === "address") saveSingleAddress(controller, value);
	if (edit === "mib")
		void applyEdit(controller, { move_in_black_enabled: value === "true" });
	if (edit === "mib_delay") {
		const seconds = Number(value);
		if (Number.isFinite(seconds))
			void applyEdit(controller, {
				move_in_black_delay_millis: Math.max(0, Math.round(seconds * 1000)),
			});
	}
	if (edit === "group_masters")
		void savePolicy(
			controller,
			{ type: "group_masters", controlled: value === "true" },
			{ group_masters_enabled: value === "true" },
		);
	if (edit === "grand_master")
		void savePolicy(
			controller,
			{ type: "grand_master", controlled: value === "true" },
			{ grand_master_enabled: value === "true" },
		);
	if (edit === "invert_pan")
		void savePolicy(
			controller,
			{
				type: "axis_inversion",
				axis: "pan",
				inverted: value === "true",
				multipatchInstanceId: null,
			},
			{ invert_pan: value === "true" },
		);
	if (edit === "invert_tilt")
		void savePolicy(
			controller,
			{
				type: "axis_inversion",
				axis: "tilt",
				inverted: value === "true",
				multipatchInstanceId: null,
			},
			{ invert_tilt: value === "true" },
		);
	if (edit === "bracket_angle") {
		const degrees = Number(value);
		if (Number.isFinite(degrees))
			void applyEdit(controller, { bracket_angle: degrees });
	}
	// Clearing the field takes the module off the fixture again; any number fits one at that
	// angle. Nothing else can say "there is no shaper here" as plainly.
	if (edit === "shaper_angle") {
		const trimmed = value.trim();
		const degrees = Number(trimmed);
		if (!trimmed) void applyEdit(controller, { shaper_angle: null });
		else if (Number.isFinite(degrees))
			void applyEdit(controller, { shaper_angle: degrees });
	}
	if (edit === "location" || edit === "rotation")
		void applyEdit(controller, {
			// A single-axis edit recomposes over the fixture's current siblings so it can
			// never resubmit a stale value for an axis it did not touch.
			[edit]: editAxis
				? {
						...(selected[edit] ?? { x: 0, y: 0, z: 0 }),
						[editAxis]: vector[editAxis],
					}
				: vector,
		});
	if (edit === "mode" && definition) {
		const highlight_overrides = compatibleHighlightOverrides(
			definition,
			selected.highlight_overrides,
		);
		void applyEdit(controller, {
			...reconcileModePatchChanges(selected, definition),
			highlight_overrides,
		});
	}
	void all;
}

export async function saveVectorSpread(
	controller: PatchController,
	kind: "location" | "rotation",
	axis: "x" | "y" | "z",
	points: number[],
) {
	const fixturesBySelectionId = new Map<string, string>();
	for (const fixture of controller.data.visible) {
		const logicalIds = fixture.logical_heads.length
			? fixture.logical_heads.map((head) => head.fixture_id)
			: [fixture.fixture_id];
		for (const id of logicalIds)
			fixturesBySelectionId.set(id, fixture.fixture_id);
	}
	const fixtureIds: string[] = [];
	const seen = new Set<string>();
	for (const selectedId of controller.selection.orderedFixtureIds ?? []) {
		const fixtureId = fixturesBySelectionId.get(selectedId);
		if (!fixtureId || seen.has(fixtureId)) continue;
		seen.add(fixtureId);
		fixtureIds.push(fixtureId);
	}
	if (fixtureIds.length < 2) {
		controller.ui.setEditError(
			"Select at least two fixtures to spread a value.",
		);
		return;
	}
	const scaledPoints =
		kind === "location" ? points.map((point) => point * 1_000) : points;
	if (
		await controller.patch.spreadFixtureVector({
			fixtureIds,
			kind,
			axis,
			points: scaledPoints,
		})
	)
		completeEdit(controller);
	else controller.ui.setEditError("The fixture spread could not be applied.");
}

export async function saveVectorAxisInput(
	controller: PatchController,
	kind: "location" | "rotation",
	axis: "x" | "y" | "z",
	value: string,
) {
	if (/\bTHRU\b/i.test(value)) {
		const points = value
			.split(/\s+THRU\s+/i)
			.map((point) => Number(point.trim()));
		if (points.length > 1 && points.every(Number.isFinite)) {
			await saveVectorSpread(controller, kind, axis, points);
			return;
		}
		controller.ui.setEditError("Enter numeric values separated by THRU.");
		return;
	}
	const parsed = Number(value);
	if (!Number.isFinite(parsed)) {
		controller.ui.setEditError("Enter a numeric value.");
		return;
	}
	if ((controller.selection.orderedFixtureIds?.length ?? 0) > 1) {
		await saveVectorSpread(controller, kind, axis, [parsed, parsed]);
		return;
	}
	const selected = controller.data.selected;
	if (!selected) return;
	await applyEdit(controller, {
		[kind]: {
			...(selected[kind] ?? { x: 0, y: 0, z: 0 }),
			[axis]: kind === "location" ? Math.round(parsed * 1_000) : parsed,
		},
	});
}

async function savePolicy(
	controller: PatchController,
	action: Parameters<typeof controller.patch.updatePolicy>[1],
	changes: Parameters<typeof controller.patch.updatePolicy>[2],
) {
	const selected = controller.data.selected;
	if (!selected) return;
	if (await controller.patch.updatePolicy(selected.fixture_id, action, changes))
		completeEdit(controller);
}

function saveFixtureNumber(controller: PatchController, value: string) {
	const { selected, all } = controller.data;
	if (!selected) return;
	if (isDmxPatchable(selected.definition)) {
		const number = parseFixtureNumber(value);
		if (
			number != null &&
			!all.some(
				(fixture) =>
					fixture.fixture_id !== selected.fixture_id &&
					fixture.fixture_number === number,
			)
		)
			void applyEdit(controller, {
				fixture_number: number,
				virtual_fixture_number: null,
			});
		return;
	}
	const number = parseVirtualFixtureNumber(value);
	if (
		number != null &&
		!all.some(
			(fixture) =>
				fixture.fixture_id !== selected.fixture_id &&
				fixture.virtual_fixture_number === number,
		)
	)
		void applyEdit(controller, {
			fixture_number: null,
			virtual_fixture_number: number,
		});
}

function saveSingleAddress(controller: PatchController, value: string) {
	const selected = controller.data.selected;
	if (!selected) return;
	const parsed = parsePatchAddress(value);
	if (selected.definition.schema_version >= 2) {
		const split = definitionSplits(selected.definition)[0]?.number ?? 1;
		if (parsed)
			void applyEdit(
				controller,
				replaceSelectedSplitPatch(
					selected.definition,
					selected.split_patches,
					selected.universe,
					selected.address,
					split,
					parsed,
				),
			);
		else if (!value.trim())
			void applyEdit(
				controller,
				replaceSelectedSplitPatch(
					selected.definition,
					selected.split_patches,
					selected.universe,
					selected.address,
					split,
					null,
				),
			);
	} else if (parsed) void applyEdit(controller, parsed);
	else if (!value.trim())
		void applyEdit(controller, { universe: null, address: null });
}

export function saveSplitEdit(controller: PatchController) {
	const selected = controller.data.selected;
	if (!selected) return;
	const parsed = parseSplitDrafts(controller);
	if (!parsed) return;
	const primary = parsed.find((item) => item.split === 1) ?? parsed[0];
	void applyEdit(controller, {
		split_patches: parsed,
		universe: primary?.universe ?? null,
		address: primary?.address ?? null,
	});
}

export function saveSelectedSplitEdit(controller: PatchController) {
	const selected = controller.data.selected;
	const split = controller.ui.editingSplit;
	if (!selected || split == null) return;
	const raw = controller.ui.editSplitDrafts[split]?.trim() ?? "";
	const value = raw ? parsePatchAddress(raw) : null;
	if (raw && !value) {
		controller.ui.setEditError(
			"Enter the split patch as universe.address, for example 1.101.",
		);
		return;
	}
	void applyEdit(
		controller,
		replaceSelectedSplitPatch(
			selected.definition,
			selected.split_patches,
			selected.universe,
			selected.address,
			split,
			value,
		),
	);
}

function parseSplitDrafts(controller: PatchController): SplitPatch[] | null {
	const selected = controller.data.selected;
	if (!selected) return null;
	const parsed = definitionSplits(selected.definition).map((split) => {
		const raw = controller.ui.editSplitDrafts[split.number]?.trim() ?? "";
		return {
			split: split.number,
			raw,
			value: raw ? parsePatchAddress(raw) : null,
		};
	});
	if (parsed.some((item) => item.raw && !item.value)) return null;
	return parsed.map((item) => ({
		split: item.split,
		universe: item.value?.universe ?? null,
		address: item.value?.address ?? null,
	}));
}
