import type { SplitPatch } from "../../../api/types";
import { parsePatchAddress } from "../../input/ConsoleFields";
import { maxRaw } from "../fixtureProfileModel";
import { compatibleHighlightOverrides, isDmxPatchable } from "../patchUtils";
import type { PatchController } from "./controller";
import { applyEdit } from "./editSession";
import { parseFixtureNumber, parseVirtualFixtureNumber } from "./fixtureIds";
import {
	definitionModeChannels,
	definitionSplits,
	reconcileModePatchChanges,
	replaceSelectedSplitPatch,
} from "./patchModel";

export function saveEdit(
	controller: PatchController,
	value = controller.ui.editText,
) {
	const { selected, all, definition } = controller.data;
	const { edit, vector } = controller.ui;
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
	if (edit === "location" || edit === "rotation")
		void applyEdit(controller, { [edit]: vector });
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

export function saveHighlightEdit(controller: PatchController) {
	const selected = controller.data.selected;
	if (!selected) return;
	const channels = new Map(
		definitionModeChannels(selected.definition).map((channel) => [
			channel.id,
			channel,
		]),
	);
	const highlight_overrides: Record<string, number> = {};
	for (const [channelId, text] of Object.entries(
		controller.ui.highlightDrafts,
	)) {
		if (!text.trim()) continue;
		const channel = channels.get(channelId);
		const raw = Number(text);
		if (
			!channel ||
			!Number.isInteger(raw) ||
			raw < 0 ||
			raw > maxRaw(channel.resolution)
		) {
			controller.ui.setEditError(
				`${channel?.attribute ?? "Highlight"} must be an exact raw value from 0 to ${channel ? maxRaw(channel.resolution) : 0}.`,
			);
			return;
		}
		highlight_overrides[channelId] = raw;
	}
	void applyEdit(controller, { highlight_overrides });
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
