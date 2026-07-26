import { parsePatchAddress } from "../../input/ConsoleFields";
import { fixtureDefinitionKey } from "../fixtureProfileModel";
import { firstFreeAddress, isDmxPatchable } from "../patchUtils";
import type { PatchController } from "./controller";
import {
	contiguousBatchPatches,
	nextAvailableFixtureNumber,
	placementBatchCount,
} from "./fixtureIds";
import { definitionSplits } from "./patchModel";

export function chooseFamily(controller: PatchController, key: string) {
	const next = controller.data.families.find((item) => item.key === key);
	if (!next) return;
	controller.ui.setFamilyKey(key);
	controller.ui.setDefinitionKey(fixtureDefinitionKey(next.modes[0]));
	controller.ui.setDraft((current) => ({ ...current, name: next.name }));
}

export function beginPlacement(controller: PatchController) {
	const { definition, all } = controller.data;
	const { ui } = controller;
	if (!definition) return;
	if (!isDmxPatchable(definition)) {
		beginVirtualPlacement(controller);
		return;
	}
	const splits = definitionSplits(definition);
	const universe = parsePatchAddress(ui.draft.patch)?.universe ?? 1;
	const address =
		firstFreeAddress(
			all,
			universe,
			splits[0]?.footprint ?? definition.footprint,
		) ?? 1;
	const nextDraft = {
		...ui.draft,
		patch: `${universe}.${address}`,
		name: ui.draft.name || definition.name,
	};
	const nextSplitDrafts = Object.fromEntries(
		splits.map((split, index) => {
			const splitAddress =
				firstFreeAddress(
					all,
					universe,
					split.footprint,
					index === 0 ? address : undefined,
				) ?? 1;
			return [split.number, `${universe}.${splitAddress}`];
		}),
	);
	ui.setDraft(nextDraft);
	ui.setSplitDrafts(nextSplitDrafts);
	ui.setBatchPatches(
		contiguousBatchPatches(
			universe,
			address,
			placementBatchCount(ui.draft.count),
			splits[0]?.footprint ?? definition.footprint,
		),
	);
	ui.setPlacementOverrides({});
	ui.setPlacementEmpty(false);
	openPlacement(controller, nextDraft, nextSplitDrafts);
}

function beginVirtualPlacement(controller: PatchController) {
	const { definition, all } = controller.data;
	const { ui } = controller;
	if (!definition) return;
	const used = new Set(
		all.flatMap((fixture) =>
			fixture.virtual_fixture_number == null
				? []
				: [fixture.virtual_fixture_number],
		),
	);
	const first = nextAvailableFixtureNumber(1, used) ?? 1;
	const nextDraft = {
		...ui.draft,
		fixtureNumber: `0.${first}`,
		patch: "",
		name: ui.draft.name || definition.name,
	};
	ui.setDraft(nextDraft);
	ui.setSplitDrafts({});
	ui.setBatchPatches([]);
	ui.setPlacementOverrides({});
	ui.setPlacementEmpty(false);
	openPlacement(controller, nextDraft, {});
}

function openPlacement(
	controller: PatchController,
	draft: PatchController["ui"]["draft"],
	splitDrafts: Record<number, string>,
) {
	const definition = controller.data.definition;
	if (!definition) return;
	controller.ui.setPlacementBaseline({
		draft,
		splitDrafts,
		definitionKey: fixtureDefinitionKey(definition),
		empty: false,
	});
	controller.ui.setPlacementCloseConfirm(false);
	controller.ui.setStatus("");
	controller.ui.setPlacementOpen(true);
}

export function updatePlacementCount(
	controller: PatchController,
	count: string,
) {
	const { ui } = controller;
	const { definition } = controller.data;
	ui.setDraft((current) => ({ ...current, count }));
	if (!definition || !isDmxPatchable(definition)) return;
	if (ui.placementEmpty) {
		ui.setBatchPatches([]);
		ui.setPlacementOverrides({});
		return;
	}
	const footprint =
		definitionSplits(definition)[0]?.footprint ?? definition.footprint;
	const base = parsePatchAddress(ui.draft.patch);
	if (!base) return;
	const nextCount = placementBatchCount(count);
	const nextOverrides = Object.fromEntries(
		Object.entries(ui.placementOverrides).filter(
			([index]) => Number(index) < nextCount,
		),
	);
	ui.setPlacementOverrides(nextOverrides);
	ui.setBatchPatches(
		batchPreviewPatches(
			base.universe,
			base.address,
			nextCount,
			footprint,
			nextOverrides,
		),
	);
}

export function updatePlacementPatch(
	controller: PatchController,
	patch: string,
) {
	const { ui } = controller;
	const { definition } = controller.data;
	ui.setDraft((current) => ({ ...current, patch }));
	ui.setPlacementEmpty(false);
	if (!definition) return;
	const parsed = parsePatchAddress(patch);
	if (parsed) {
		ui.setPlacementOverrides({});
		ui.setBatchPatches(
			contiguousBatchPatches(
				parsed.universe,
				parsed.address,
				placementBatchCount(ui.draft.count),
				definitionSplits(definition)[0]?.footprint ?? definition.footprint,
			),
		);
	}
}

export function updateSplitPlacementPatch(
	controller: PatchController,
	split: number,
	value: string,
) {
	const { ui } = controller;
	const { definition } = controller.data;
	ui.setSplitDrafts((current) => ({ ...current, [split]: value }));
	if (!definition || split !== definitionSplits(definition)[0]?.number) return;
	const parsed = parsePatchAddress(value);
	if (!parsed) return;
	ui.setDraft((current) => ({ ...current, patch: value }));
	ui.setPlacementOverrides({});
	ui.setBatchPatches(
		contiguousBatchPatches(
			parsed.universe,
			parsed.address,
			placementBatchCount(ui.draft.count),
			definitionSplits(definition)[0].footprint,
		),
	);
}

export function updateBatchPatch(
	controller: PatchController,
	index: number,
	universe: number,
	address: number,
) {
	const value = `${universe}.${address}`;
	const { ui } = controller;
	ui.setBatchPatches((current) =>
		current.map((patch, candidate) => (candidate === index ? value : patch)),
	);
	ui.setPlacementOverrides((current) => ({ ...current, [index]: value }));
	if (index === 0) {
		ui.setDraft((current) => ({ ...current, patch: value }));
		const firstSplit = definitionSplits(controller.data.definition)[0]?.number;
		if (firstSplit != null)
			ui.setSplitDrafts((current) => ({ ...current, [firstSplit]: value }));
	}
}

export function reattachPlacementBlock(controller: PatchController) {
	const { definition } = controller.data;
	const { ui } = controller;
	if (!definition) return;
	const anchor = parsePatchAddress(ui.batchPatches[0] ?? ui.draft.patch);
	if (!anchor) return;
	const footprint =
		definitionSplits(definition)[0]?.footprint ?? definition.footprint;
	const patches = contiguousBatchPatches(
		anchor.universe,
		anchor.address,
		placementBatchCount(ui.draft.count),
		footprint,
	);
	ui.setBatchPatches(patches);
	ui.setPlacementOverrides({});
	ui.setDraft((current) => ({ ...current, patch: patches[0] }));
}

export function changePlacementUniverse(
	controller: PatchController,
	universe: number,
) {
	const { definition, all } = controller.data;
	const { ui } = controller;
	if (!definition) return;
	const address = firstFreeAddress(all, universe, definition.footprint) ?? 1;
	const patches = contiguousBatchPatches(
		universe,
		address,
		placementBatchCount(ui.draft.count),
		definitionSplits(definition)[0]?.footprint ?? definition.footprint,
	);
	ui.setBatchPatches(patches);
	ui.setPlacementOverrides({});
	ui.setDraft({ ...ui.draft, patch: patches[0] });
	ui.setSplitDrafts((current) => ({
		...current,
		[definitionSplits(definition)[0]?.number ?? 1]: patches[0],
	}));
	ui.setPlacementEmpty(false);
}

export function setPlacementEmpty(controller: PatchController, empty: boolean) {
	const { definition, all } = controller.data;
	const { ui } = controller;
	if (!definition || !isDmxPatchable(definition)) return;
	ui.setPlacementEmpty(empty);
	ui.setPlacementOverrides({});
	if (empty) {
		ui.setBatchPatches([]);
		return;
	}
	const splits = definitionSplits(definition);
	const retainedPrimary = parsePatchAddress(
		splits.length > 1
			? (ui.splitDrafts[splits[0].number] ?? "")
			: ui.draft.patch,
	);
	const universe = retainedPrimary?.universe ?? 1;
	const address =
		retainedPrimary?.address ??
		firstFreeAddress(
			all,
			universe,
			splits[0]?.footprint ?? definition.footprint,
		) ??
		1;
	const patch = `${universe}.${address}`;
	ui.setDraft((current) => ({ ...current, patch }));
	ui.setSplitDrafts(
		Object.fromEntries(
			splits.map((split, index) => {
				const retained = parsePatchAddress(ui.splitDrafts[split.number] ?? "");
				const splitAddress =
					retained?.address ??
					firstFreeAddress(
						all,
						universe,
						split.footprint,
						index === 0 ? address : undefined,
					) ??
					1;
				return [
					split.number,
					`${retained?.universe ?? universe}.${splitAddress}`,
				];
			}),
		),
	);
	ui.setBatchPatches(
		contiguousBatchPatches(
			universe,
			address,
			placementBatchCount(ui.draft.count),
			splits[0]?.footprint ?? definition.footprint,
		),
	);
}

function batchPreviewPatches(
	universe: number,
	address: number,
	count: number,
	footprint: number,
	overrides: Record<number, string>,
) {
	return contiguousBatchPatches(universe, address, count, footprint).map(
		(patch, index) => overrides[index] ?? patch,
	);
}

export function placementIsDirty(controller: PatchController) {
	const {
		placementBaseline,
		draft,
		splitDrafts,
		placementOverrides,
		placementEmpty,
	} = controller.ui;
	const { definition } = controller.data;
	return Boolean(
		placementBaseline &&
			definition &&
			(placementBaseline.definitionKey !== fixtureDefinitionKey(definition) ||
				JSON.stringify(placementBaseline.draft) !== JSON.stringify(draft) ||
				JSON.stringify(placementBaseline.splitDrafts) !==
					JSON.stringify(splitDrafts) ||
				placementBaseline.empty !== placementEmpty ||
				Object.keys(placementOverrides).length > 0),
	);
}

export function closePlacement(controller: PatchController) {
	controller.ui.setPlacementOpen(false);
	controller.ui.setPlacementAddressOpen(false);
	controller.ui.setPlacementCloseConfirm(false);
	controller.ui.setPlacementBaseline(null);
	controller.ui.setStatus("");
}

export function requestPlacementClose(controller: PatchController) {
	if (placementIsDirty(controller))
		controller.ui.setPlacementCloseConfirm(true);
	else closePlacement(controller);
}
