import type { SplitPatch } from "../../../api/types";
import type { PatchFixtureUpdateAction } from "../../../features/patch/contracts";
import { parsePatchAddress } from "../../input/ConsoleFields";
import { compatibleHighlightOverrides, isVisualOnly } from "../patchUtils";
import {
	type CombinedPolicyChoice,
	combinedPolicyValues,
} from "./combinedPolicy";
import type { PatchController } from "./controller";
import { applyEdit, completeEdit } from "./editSession";
import { parseFixtureNumber, parseVirtualFixtureNumber } from "./fixtureIds";
import {
	definitionSplits,
	fixturePolicyApplicability,
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
	if (edit === "mib") {
		const result = parseMibInput(value);
		if ("error" in result) controller.ui.setEditError(result.error);
		else
			void applyFixtureIntent(controller, {
				type: "set_move_in_black",
				enabled: result.changes.move_in_black_enabled,
				delayMillis: result.changes.move_in_black_delay_millis,
			});
	}
	if (edit === "masters") {
		const choice = combinedPolicyValues(value as CombinedPolicyChoice);
		const applicable = fixturePolicyApplicability(selected.definition);
		void applyFixtureIntent(controller, {
			type: "set_masters",
			groupMastersEnabled: applicable.groupMasters
				? choice.first
				: (selected.group_masters_enabled ?? true),
			grandMasterEnabled: applicable.grandMaster
				? choice.second
				: (selected.grand_master_enabled ?? true),
		});
	}
	if (edit === "pan_tilt") {
		const choice = combinedPolicyValues(value as CombinedPolicyChoice);
		const applicable = fixturePolicyApplicability(selected.definition);
		void applyFixtureIntent(controller, {
			type: "set_pan_tilt",
			invertPan: applicable.pan ? choice.first : (selected.invert_pan ?? false),
			invertTilt: applicable.tilt
				? choice.second
				: (selected.invert_tilt ?? false),
		});
	}
	if (edit === "bracket_angle") {
		const degrees = Number(value);
		if (Number.isFinite(degrees))
			void applyFixtureIntent(controller, {
				type: "set_bracket_angle",
				degrees,
			});
	}
	// Clearing the field takes the module off the fixture again; any number fits one at that
	// angle. Nothing else can say "there is no shaper here" as plainly.
	if (edit === "shaper_angle") {
		const trimmed = value.trim();
		const degrees = Number(trimmed);
		if (!trimmed)
			void applyFixtureIntent(controller, {
				type: "set_shaper_module_rotation",
				degrees: null,
			});
		else if (Number.isFinite(degrees))
			void applyFixtureIntent(controller, {
				type: "set_shaper_module_rotation",
				degrees,
			});
	}
	if (edit === "internal_bindings") {
		const bindings = parseInternalBindingDraft(value);
		if (!bindings) {
			controller.ui.setEditError("Enter valid logical library and output names.");
		} else {
			void applyEdit(controller, { internal_bindings: bindings });
		}
	}
	if (edit === "crowd_width" || edit === "crowd_depth")
		void saveCrowdFootprint(controller, edit, value);
	if ((edit === "location" || edit === "rotation") && editAxis)
		void applyFixtureIntent(
			controller,
			edit === "location"
				? {
						type: "set_location_axis",
						axis: editAxis,
						millimetres: vector[editAxis],
					}
				: {
						type: "set_rotation_axis",
						axis: editAxis,
						degrees: vector[editAxis],
					},
		);
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

async function saveCrowdFootprint(
	controller: PatchController,
	kind: "crowd_width" | "crowd_depth",
	value: string,
) {
	const selected = controller.data.selected;
	const crowd = selected?.definition.profile_snapshot?.crowd;
	const actions = controller.stageActions;
	if (!selected || !crowd || !actions?.canWrite) {
		controller.ui.setEditError("The Crowd Area footprint cannot be edited right now.");
		return;
	}
	const parsed = Number(value);
	if (!Number.isFinite(parsed) || parsed < 1 || parsed > 250) {
		controller.ui.setEditError("Enter a crowd footprint from 1 to 250 metres.");
		return;
	}
	const stored = controller.stagePositions3d[selected.fixture_id];
	const width =
		kind === "crowd_width"
			? parsed
			: (stored?.crowdWidthMetres ?? crowd.default_width_metres);
	const depth =
		kind === "crowd_depth"
			? parsed
			: (stored?.crowdDepthMetres ?? crowd.default_depth_metres);
	try {
		await actions.setCrowdFootprint(selected.fixture_id, width, depth);
		completeEdit(controller);
	} catch (error) {
		controller.ui.setEditError(
			error instanceof Error ? error.message : "The crowd footprint could not be saved.",
		);
	}
}

export function parseInternalBindingDraft(value: string) {
	try {
		const draft = JSON.parse(value) as { library?: unknown; output?: unknown };
		const normalize = (candidate: unknown) => {
			if (typeof candidate !== "string") return undefined;
			const trimmed = candidate.trim();
			return trimmed || undefined;
		};
		const library = normalize(draft.library);
		const output = normalize(draft.output);
		if ((library?.length ?? 0) > 128 || (output?.length ?? 0) > 128)
			return null;
		return { library, output };
	} catch {
		return null;
	}
}

async function applyFixtureIntent(
	controller: PatchController,
	action: PatchFixtureUpdateAction,
) {
	const selected = controller.data.selected;
	if (!selected) return;
	controller.ui.setEditError("");
	if (
		await controller.patch.updateFixtureIntent(
			selected.fixture_id,
			null,
			action,
		)
	)
		completeEdit(controller);
	else controller.ui.setEditError("The fixture update could not be applied.");
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
	await applyFixtureIntent(
		controller,
		kind === "location"
			? {
					type: "set_location_axis",
					axis,
					millimetres: Math.round(parsed * 1_000),
				}
			: { type: "set_rotation_axis", axis, degrees: parsed },
	);
}

export function parseMibInput(value: string):
	| {
			changes: {
				move_in_black_enabled: boolean;
				move_in_black_delay_millis: number;
			};
	  }
	| { error: string } {
	const trimmed = value.trim();
	if (trimmed.toLowerCase() === "off")
		return {
			changes: {
				move_in_black_enabled: false,
				move_in_black_delay_millis: 0,
			},
		};
	if (!/^(?:\d+(?:\.\d*)?|\.\d+)$/.test(trimmed))
		return { error: "Enter Off or a finite non-negative delay in seconds." };
	const seconds = Number(trimmed);
	if (!Number.isFinite(seconds))
		return { error: "Enter Off or a finite non-negative delay in seconds." };
	const milliseconds = Math.round(seconds * 1_000);
	if (!Number.isSafeInteger(milliseconds))
		return { error: "The MIB delay is too large to store safely." };
	return {
		changes: {
			move_in_black_enabled: true,
			move_in_black_delay_millis: milliseconds,
		},
	};
}

function saveFixtureNumber(controller: PatchController, value: string) {
	const { selected, all } = controller.data;
	if (!selected) return;
	if (!isVisualOnly(selected.definition)) {
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
