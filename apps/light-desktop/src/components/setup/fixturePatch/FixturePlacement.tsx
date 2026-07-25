import { Fragment } from "react";
import { Button, Select } from "../../common";
import {
	ConsoleNumberField,
	ConsoleTextField,
	parsePatchAddress,
} from "../../input/ConsoleFields";
import { fixtureDefinitionKey } from "../fixtureProfileModel";
import { incrementFixtureName, isDmxPatchable } from "../patchUtils";
import { type PatchController, usePatchController } from "./controller";
import {
	batchPatchError,
	parseFixtureNumber,
	parseVirtualFixtureNumber,
	placementBatchCount,
} from "./fixtureIds";
import { definitionSplits } from "./patchModel";
import { addPlacementBatch } from "./placementBatch";
import {
	changePlacementUniverse,
	requestPlacementClose,
	updateBatchPatch,
	updatePlacementCount,
	updatePlacementPatch,
} from "./placementDraft";
import { UniverseMap } from "./UniverseMap";

export function FixturePlacement() {
	const controller = usePatchController();
	const definition = controller.data.definition;
	if (!controller.ui.placementOpen || !definition) return null;
	return (
		<div className="stacked-modal-layer">
			<section className="nested-modal fixture-placement-modal">
				<PlacementHeader controller={controller} />
				<div className="placement-grid">
					<PlacementFields controller={controller} />
					{isDmxPatchable(definition) && (
						<PlacementUniverseMap controller={controller} />
					)}
				</div>
			</section>
		</div>
	);
}

function PlacementHeader({ controller }: { controller: PatchController }) {
	const { definition, family } = controller.data;
	if (!definition) return null;
	return (
		<header>
			<h2>
				{isDmxPatchable(definition) ? "Patch" : "Add"} {family?.name}
			</h2>
			<Button onClick={() => requestPlacementClose(controller)}>Cancel</Button>
			<Button
				className="primary"
				disabled={placementIsDisabled(controller)}
				onClick={() => void addPlacementBatch(controller)}
			>
				{controller.ui.busy
					? "Adding…"
					: `Add ${controller.ui.draft.count || 1} fixtures`}
			</Button>
			<Button
				className="modal-close"
				aria-label="Close Add Fixture"
				onClick={() => requestPlacementClose(controller)}
			>
				×
			</Button>
		</header>
	);
}

function PlacementFields({ controller }: { controller: PatchController }) {
	const { definition, family } = controller.data;
	const { ui } = controller;
	if (!definition) return null;
	return (
		<div className="placement-fields">
			{/* biome-ignore lint/a11y/noLabelWithoutControl: Select renders its native control inside this label. */}
			<label>
				Mode
				<Select
					aria-label="Mode"
					value={fixtureDefinitionKey(definition)}
					onChange={(event) => ui.setDefinitionKey(event.target.value)}
				>
					{family?.modes.map((mode) => (
						<option
							value={fixtureDefinitionKey(mode)}
							key={fixtureDefinitionKey(mode)}
						>
							{mode.mode} ·{" "}
							{isDmxPatchable(mode) ? `${mode.footprint}ch` : "No DMX"}
						</option>
					))}
				</Select>
			</label>
			{/* biome-ignore lint/a11y/noLabelWithoutControl: ConsoleTextField renders its native control inside this label. */}
			<label>
				Fixture name
				<ConsoleTextField
					label="Fixture name"
					autoFocus
					value={ui.draft.name}
					onChange={(name) => ui.setDraft({ ...ui.draft, name })}
				/>
			</label>
			<small>Trailing numbers increment automatically.</small>
			{/* biome-ignore lint/a11y/noLabelWithoutControl: ConsoleNumberField renders its native control inside this label. */}
			<label>
				Start fixture ID
				<ConsoleNumberField
					label="Start fixture ID"
					allowDecimal={!isDmxPatchable(definition)}
					value={ui.draft.fixtureNumber}
					onChange={(fixtureNumber) =>
						ui.setDraft({ ...ui.draft, fixtureNumber })
					}
				/>
			</label>
			<small>Taken fixture IDs are skipped automatically.</small>
			{/* biome-ignore lint/a11y/noLabelWithoutControl: ConsoleNumberField renders its native control inside this label. */}
			<label>
				Count
				<ConsoleNumberField
					label="Count"
					value={ui.draft.count}
					onChange={(count) => updatePlacementCount(controller, count)}
				/>
			</label>
			<PlacementPatchFields controller={controller} />
			{ui.status && <p className="patch-status">{ui.status}</p>}
		</div>
	);
}

function PlacementPatchFields({ controller }: { controller: PatchController }) {
	const definition = controller.data.definition;
	const { ui } = controller;
	if (!definition) return null;
	if (!isDmxPatchable(definition))
		return <p>This Venue element is visual only and has no DMX patch.</p>;
	if (definitionSplits(definition).length === 1)
		return (
			// biome-ignore lint/a11y/noLabelWithoutControl: ConsoleTextField renders its native control inside this label.
			<label>
				Address (universe.address)
				<ConsoleTextField
					label="Address (universe.address)"
					value={ui.draft.patch}
					onChange={(patch) => updatePlacementPatch(controller, patch)}
				/>
			</label>
		);
	return (
		<fieldset className="split-patch-fields">
			<legend>Independent split patches</legend>
			{definitionSplits(definition).map((split) => (
				<Fragment key={split.number}>
					{/* biome-ignore lint/a11y/noLabelWithoutControl: ConsoleTextField renders its native control inside this label. */}
					<label>
						Split {split.number} · {split.footprint} slots
						<ConsoleTextField
							label={`Split ${split.number} address`}
							value={ui.splitDrafts[split.number] ?? ""}
							onChange={(value) =>
								ui.setSplitDrafts((current) => ({
									...current,
									[split.number]: value,
								}))
							}
						/>
						<small>Clear to leave this split unpatched.</small>
					</label>
				</Fragment>
			))}
		</fieldset>
	);
}

function PlacementUniverseMap({ controller }: { controller: PatchController }) {
	const { definition, all, shownUniverse, shownAddress } = controller.data;
	const { ui } = controller;
	if (!definition) return null;
	const footprint =
		definitionSplits(definition)[0]?.footprint ?? definition.footprint;
	return (
		<UniverseMap
			fixtures={all}
			universe={shownUniverse}
			proposed={shownAddress}
			footprint={footprint}
			proposedLabel={`Fixture ${ui.draft.fixtureNumber || "—"} · ${ui.draft.name || definition.name}`}
			proposals={placementProposals(controller, footprint)}
			onAddress={(address) =>
				updateBatchPatch(controller, 0, shownUniverse, address)
			}
			onProposalAddress={(key, address) =>
				updateBatchPatch(controller, Number(key), shownUniverse, address)
			}
			onUniverse={(universe) => changePlacementUniverse(controller, universe)}
		/>
	);
}

function placementProposals(controller: PatchController, footprint: number) {
	const { definition, shownUniverse } = controller.data;
	const { ui } = controller;
	if (!definition) return [];
	return ui.batchPatches
		.map((patch) => parsePatchAddress(patch))
		.filter((patch): patch is { universe: number; address: number } =>
			Boolean(patch && patch.universe === shownUniverse),
		)
		.map((patch, index) => ({
			key: String(index),
			start: patch.address,
			footprint,
			label: `Fixture ${(parseFixtureNumber(ui.draft.fixtureNumber) ?? 1) + index} · ${incrementFixtureName(ui.draft.name || definition.name, index)}`,
		}));
}

function placementIsDisabled(controller: PatchController) {
	const { definition, all } = controller.data;
	const { ui } = controller;
	if (!definition || ui.busy) return true;
	if (isDmxPatchable(definition)) {
		if (parseFixtureNumber(ui.draft.fixtureNumber) == null) return true;
		if (definitionSplits(definition).length === 1)
			return (
				batchPatchError(
					ui.batchPatches
						.slice(0, placementBatchCount(ui.draft.count))
						.map(parsePatchAddress),
					definition.footprint,
					all,
				) != null
			);
		return definitionSplits(definition).some(
			(split) =>
				Boolean(ui.splitDrafts[split.number]?.trim()) &&
				!parsePatchAddress(ui.splitDrafts[split.number]),
		);
	}
	return parseVirtualFixtureNumber(ui.draft.fixtureNumber) == null;
}
