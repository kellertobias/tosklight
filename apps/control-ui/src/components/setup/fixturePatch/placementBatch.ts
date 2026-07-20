import type { SplitPatch } from "../../../api/types";
import {
	newPatchFixtureCandidate,
	type PatchFixtureCandidate,
} from "../../../features/patch/PatchContext";
import { parsePatchAddress } from "../../input/ConsoleFields";
import { conflicts, incrementFixtureName, isDmxPatchable } from "../patchUtils";
import type { PatchController } from "./controller";
import {
	batchPatchError,
	nextAvailableFixtureNumber,
	parseFixtureNumber,
	parseVirtualFixtureNumber,
	placementBatchCount,
} from "./fixtureIds";
import { definitionSplits, splitPatchSetError } from "./patchModel";

export async function addPlacementBatch(controller: PatchController) {
	const definition = controller.data.definition;
	if (!definition) return;
	if (!isDmxPatchable(definition)) {
		await addVirtualBatch(controller);
		return;
	}
	if (definitionSplits(definition).length > 1) {
		await addSplitBatch(controller);
		return;
	}
	await addSingleSplitBatch(controller);
}

async function addVirtualBatch(controller: PatchController) {
	const { definition, all } = controller.data;
	const { ui } = controller;
	if (!definition) return;
	const fixtureNumber = parseVirtualFixtureNumber(ui.draft.fixtureNumber);
	if (fixtureNumber == null) {
		ui.setStatus("Enter a virtual fixture ID starting at 0.1.");
		return;
	}
	const requested = placementBatchCount(ui.draft.count);
	const candidates: PatchFixtureCandidate[] = [];
	let fixtureNumberCursor = fixtureNumber;
	const usedFixtureNumbers = new Set(
		all.flatMap((fixture) =>
			fixture.virtual_fixture_number == null
				? []
				: [fixture.virtual_fixture_number],
			),
	);
	while (candidates.length < requested) {
		const nextFixtureNumber = nextAvailableFixtureNumber(
			fixtureNumberCursor,
			usedFixtureNumbers,
		);
		if (nextFixtureNumber == null) break;
		candidates.push(
			newPatchFixtureCandidate({
				name: incrementFixtureName(ui.draft.name, candidates.length),
				fixture_number: null,
				virtual_fixture_number: nextFixtureNumber,
				definition,
				universe: null,
				address: null,
				split_patches: definitionSplits(definition).map((split) => ({
					split: split.number,
					universe: null,
					address: null,
				})),
				layer_id: ui.activeLayer === "all" ? "default" : ui.activeLayer,
			}),
		);
		usedFixtureNumbers.add(nextFixtureNumber);
		fixtureNumberCursor = nextFixtureNumber + 1;
	}
	const ids = await commitPlacementBatch(controller, candidates);
	if (!ids) return;
	const remaining = requested - candidates.length;
	const lastId = ids.at(-1) ?? null;
	if (lastId) ui.setSelectedFixture(lastId);
	if (!remaining) closeCompletedBatch(controller);
}

async function addSingleSplitBatch(controller: PatchController) {
	const { definition, all } = controller.data;
	const { ui } = controller;
	if (!definition) return;
	const fixtureNumber = parseFixtureNumber(ui.draft.fixtureNumber);
	if (fixtureNumber == null) {
		ui.setStatus("Enter a positive whole-number start fixture ID.");
		return;
	}
	const planned = ui.batchPatches
		.slice(0, placementBatchCount(ui.draft.count))
		.map(parsePatchAddress);
	const plannedError = batchPatchError(planned, definition.footprint, all);
	if (plannedError) {
		ui.setStatus(plannedError);
		return;
	}
	const candidates: PatchFixtureCandidate[] = [];
	let fixtureNumberCursor = fixtureNumber;
	const usedFixtureNumbers = physicalFixtureNumbers(all);
	for (const patch of planned) {
		if (!patch) break;
		const nextFixtureNumber = nextAvailableFixtureNumber(
			fixtureNumberCursor,
			usedFixtureNumbers,
		);
		if (nextFixtureNumber == null) break;
		candidates.push(
			newPatchFixtureCandidate({
				name: incrementFixtureName(ui.draft.name, candidates.length),
				fixture_number: nextFixtureNumber,
				definition,
				universe: patch.universe,
				address: patch.address,
				layer_id: ui.activeLayer === "all" ? "default" : ui.activeLayer,
			}),
		);
		usedFixtureNumbers.add(nextFixtureNumber);
		fixtureNumberCursor = nextFixtureNumber + 1;
	}
	const ids = await commitPlacementBatch(controller, candidates);
	if (!ids) return;
	const added = candidates.length;
	const remaining = planned.length - added;
	const lastId = ids.at(-1) ?? null;
	selectLastAdded(controller, lastId);
	if (!remaining) {
		closeCompletedBatch(controller);
		return;
	}
	continueSingleBatch(
		controller,
		remaining,
		added,
		fixtureNumberCursor,
		usedFixtureNumbers,
	);
}

async function addSplitBatch(controller: PatchController) {
	const { definition, all } = controller.data;
	const { ui } = controller;
	if (!definition) return;
	const fixtureNumber = parseFixtureNumber(ui.draft.fixtureNumber);
	if (fixtureNumber == null) {
		ui.setStatus("Enter a positive whole-number start fixture ID.");
		return;
	}
	const splits = definitionSplits(definition);
	const parsed = splits.map((split) => {
		const raw = ui.splitDrafts[split.number]?.trim() ?? "";
		return { split, raw, address: raw ? parsePatchAddress(raw) : null };
	});
	if (parsed.some((item) => item.raw && !item.address)) {
		ui.setStatus("Enter split patches as universe.address, for example 1.101.");
		return;
	}
	const requested = placementBatchCount(ui.draft.count);
	const candidates: PatchFixtureCandidate[] = [];
	let fixtureNumberCursor = fixtureNumber;
	const usedFixtureNumbers = physicalFixtureNumbers(all);
	const addresses = parsed.map((item) => ({
		split: item.split,
		universe: item.address?.universe ?? null,
		address: item.address?.address ?? null,
	}));
	const initialError = splitPatchSetError(definition, splitPatches(addresses));
	if (initialError) {
		ui.setStatus(initialError);
		return;
	}
	while (candidates.length < requested) {
		const plannedPrimary = parsePatchAddress(
			ui.batchPatches[candidates.length] ?? "",
		);
		if (plannedPrimary) {
			addresses[0].universe = plannedPrimary.universe;
			addresses[0].address = plannedPrimary.address;
		}
		if (
			splitPatchSetError(definition, splitPatches(addresses)) ||
			!addresses.every(
				(item) =>
					item.address == null ||
					item.universe == null ||
					!conflicts(all, item.universe, item.address, item.split.footprint)
						.length,
			)
		)
			break;
		const nextFixtureNumber = nextAvailableFixtureNumber(
			fixtureNumberCursor,
			usedFixtureNumbers,
		);
		if (nextFixtureNumber == null) break;
		const patches = splitPatches(addresses);
		const primary = patches.find((item) => item.split === 1) ?? patches[0];
		candidates.push(
			newPatchFixtureCandidate({
				name: incrementFixtureName(ui.draft.name, candidates.length),
				fixture_number: nextFixtureNumber,
				definition,
				universe: primary?.universe ?? null,
				address: primary?.address ?? null,
				split_patches: patches,
				layer_id: ui.activeLayer === "all" ? "default" : ui.activeLayer,
			}),
		);
		usedFixtureNumbers.add(nextFixtureNumber);
		fixtureNumberCursor = nextFixtureNumber + 1;
		addresses.forEach((item, index) => {
			if (index > 0 && item.address != null)
				item.address += item.split.footprint;
		});
	}
	const ids = await commitPlacementBatch(controller, candidates);
	if (!ids) return;
	const added = candidates.length;
	const remaining = requested - added;
	const lastId = ids.at(-1) ?? null;
	selectLastAdded(controller, lastId);
	if (!remaining) {
		closeCompletedBatch(controller);
		return;
	}
	continueSplitBatch(
		controller,
		addresses,
		remaining,
		added,
		fixtureNumberCursor,
		usedFixtureNumbers,
	);
}

function continueSingleBatch(
	controller: PatchController,
	remaining: number,
	added: number,
	fixtureNumberCursor: number,
	usedFixtureNumbers: Set<number>,
) {
	const { ui } = controller;
	const nextFixtureNumber = nextAvailableFixtureNumber(
		fixtureNumberCursor,
		usedFixtureNumbers,
	);
	const nextPatches = ui.batchPatches.slice(added);
	ui.setBatchPatches(nextPatches);
	ui.setDraft((current) => ({
		...current,
		fixtureNumber: String(nextFixtureNumber ?? fixtureNumberCursor),
		count: String(remaining),
		patch: nextPatches[0] ?? current.patch,
	}));
	ui.setStatus(partialBatchStatus(added, remaining));
}

function continueSplitBatch(
	controller: PatchController,
	addresses: Array<{
		split: { number: number; footprint: number };
		universe: number | null;
		address: number | null;
	}>,
	remaining: number,
	added: number,
	fixtureNumberCursor: number,
	usedFixtureNumbers: Set<number>,
) {
	continueSingleBatch(
		controller,
		remaining,
		added,
		fixtureNumberCursor,
		usedFixtureNumbers,
	);
	controller.ui.setSplitDrafts(
		Object.fromEntries(
			addresses.map((item) => [
				item.split.number,
				item.universe != null && item.address != null
					? `${item.universe}.${item.address}`
					: "",
			]),
		),
	);
}

function splitPatches(
	addresses: Array<{
		split: { number: number };
		universe: number | null;
		address: number | null;
	}>,
): SplitPatch[] {
	return addresses.map((item) => ({
		split: item.split.number,
		universe: item.universe,
		address: item.address,
	}));
}

function physicalFixtureNumbers(fixtures: PatchController["data"]["all"]) {
	return new Set(
		fixtures.flatMap((fixture) =>
			fixture.fixture_number == null ? [] : [fixture.fixture_number],
		),
	);
}

async function commitPlacementBatch(
	controller: PatchController,
	candidates: readonly PatchFixtureCandidate[],
): Promise<string[] | null> {
	if (!candidates.length) {
		controller.ui.setStatus("No available fixture IDs could be added.");
		return null;
	}
	controller.ui.setBusy(true);
	try {
		const ids = await controller.patch.patchFixtures(candidates);
		if (!ids)
			controller.ui.setStatus(
				"Fixtures could not be added. Review the Patch status and try again.",
			);
		return ids;
	} catch {
		controller.ui.setStatus(
			"Fixtures could not be added. Review the Patch status and try again.",
		);
		return null;
	} finally {
		controller.ui.setBusy(false);
	}
}

function selectLastAdded(controller: PatchController, lastId: string | null) {
	if (!lastId) return;
	controller.ui.setSelectedFixture(lastId);
	void controller.selection.actions?.replace({ resolvedFixtures: [lastId] });
}

function closeCompletedBatch(controller: PatchController) {
	controller.ui.setPlacementOpen(false);
	controller.ui.setBrowserOpen(false);
	controller.ui.setStatus("");
}

function partialBatchStatus(added: number, remaining: number) {
	return `${added} fixture${added === 1 ? "" : "s"} added. Choose where to patch the remaining ${remaining}.`;
}
