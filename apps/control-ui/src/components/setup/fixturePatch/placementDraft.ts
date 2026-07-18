import { parsePatchAddress } from "../../input/ConsoleFields";
import { fixtureDefinitionKey } from "../fixtureProfileModel";
import { firstFreeAddress, isDmxPatchable } from "../patchUtils";
import type { PatchController } from "./controller";
import {
	contiguousBatchPatches,
	nextAvailableFixtureNumber,
	placementBatchCount,
	resizeBatchPatches,
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
	const footprint =
		definitionSplits(definition)[0]?.footprint ?? definition.footprint;
	const base = parsePatchAddress(ui.batchPatches[0] ?? ui.draft.patch);
	if (!base) return;
	ui.setBatchPatches((current) =>
		resizeBatchPatches(
			current,
			placementBatchCount(count),
			base.universe,
			base.address,
			footprint,
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
	if (!definition) return;
	const parsed = parsePatchAddress(patch);
	if (parsed)
		ui.setBatchPatches(
			contiguousBatchPatches(
				parsed.universe,
				parsed.address,
				placementBatchCount(ui.draft.count),
				definitionSplits(definition)[0]?.footprint ?? definition.footprint,
			),
		);
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
	const { definition } = controller.data;
	ui.setBatchPatches((current) =>
		current.map((patch, candidate) => (candidate === index ? value : patch)),
	);
	if (index !== 0) return;
	ui.setDraft((current) => ({ ...current, patch: value }));
	const primarySplit = definition && definitionSplits(definition)[0]?.number;
	if (primarySplit != null)
		ui.setSplitDrafts((current) => ({ ...current, [primarySplit]: value }));
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
	ui.setDraft({ ...ui.draft, patch: patches[0] });
	ui.setSplitDrafts((current) => ({
		...current,
		[definitionSplits(definition)[0]?.number ?? 1]: patches[0],
	}));
}

export function placementIsDirty(controller: PatchController) {
	const { placementBaseline, draft, splitDrafts } = controller.ui;
	const { definition } = controller.data;
	return Boolean(
		placementBaseline &&
			definition &&
			(placementBaseline.definitionKey !== fixtureDefinitionKey(definition) ||
				JSON.stringify(placementBaseline.draft) !== JSON.stringify(draft) ||
				JSON.stringify(placementBaseline.splitDrafts) !==
					JSON.stringify(splitDrafts)),
	);
}

export function closePlacement(controller: PatchController) {
	controller.ui.setPlacementOpen(false);
	controller.ui.setPlacementCloseConfirm(false);
	controller.ui.setPlacementBaseline(null);
	controller.ui.setStatus("");
}

export function requestPlacementClose(controller: PatchController) {
	if (placementIsDirty(controller))
		controller.ui.setPlacementCloseConfirm(true);
	else closePlacement(controller);
}
