import { useState } from "react";
import { ModalTitleBar, TextInput } from "@tosklight/ui";
import { parsePatchAddress } from "../fields";
import { incrementFixtureName } from "../patchUtils";
import type { PatchController } from "./controller";
import { DmxAddressField } from "./DmxAddressField";
import { parseFixtureNumber, placementBatchCount } from "./fixtureIds";
import { definitionSplits } from "./patchModel";
import {
	changePlacementUniverse,
	reattachPlacementBlock,
	updateBatchPatch,
} from "./placementDraft";
import { UniverseMap } from "./UniverseMap";

export function BatchFixtureAddressScreen({
	controller,
	onClose,
}: {
	controller: PatchController;
	onClose: () => void;
}) {
	const { definition, all, shownUniverse } = controller.data;
	const { ui } = controller;
	const [selectedKey, setSelectedKey] = useState("0");
	if (!definition) return null;
	const footprint =
		definitionSplits(definition)[0]?.footprint ?? definition.footprint;
	const proposals = ui.batchPatches
		.map((patch) => parsePatchAddress(patch))
		.map((patch, index) =>
			patch && patch.universe === shownUniverse
				? {
						key: String(index),
						start: patch.address,
						footprint,
						label: fixtureLabel(controller, index),
					}
				: null,
		)
		.filter((proposal): proposal is NonNullable<typeof proposal> =>
			Boolean(proposal),
		);
	const selectedIndex = Math.min(
		Number(selectedKey) || 0,
		Math.max(0, ui.batchPatches.length - 1),
	);
	const selectedValue = ui.batchPatches[selectedIndex] ?? "";
	const selectedAddress = parsePatchAddress(selectedValue);
	const detachedCount = Object.keys(ui.placementOverrides).length;
	return (
		<section
			className="nested-modal fixture-address-screen batch-address-screen"
			role="dialog"
			aria-modal="true"
			aria-label="Fixture addresses"
		>
			<ModalTitleBar
				title="Fixture addresses"
				details={`${placementBatchCount(ui.draft.count)} fixtures · ${footprint} slots each`}
				groups={[
					{
						id: "arrange",
						actions: [
							{
								id: "arrange",
								label: "Arrange as block",
								disabled: !detachedCount,
								onPress: () => reattachPlacementBlock(controller),
							},
						],
					},
				]}
				accept={{
					id: "done",
					label: "Done",
					variant: "primary",
					onPress: onClose,
				}}
				closeLabel="Close fixture addresses"
				onClose={onClose}
			/>
			<div className="fixture-address-summary">
				<span>
					Placement{" "}
					<b>{detachedCount ? "Independent fixtures" : "Attached block"}</b>
				</span>
				<span>
					Block size{" "}
					<b>{placementBatchCount(ui.draft.count) * footprint} slots</b>
				</span>
				<span>
					Universe <b>{shownUniverse}</b>
				</span>
				<span>
					Selected <b>{fixtureLabel(controller, selectedIndex)}</b>
				</span>
			</div>
			<div className="fixture-address-content">
				<div className="batch-address-list">
					<TextInput
						aria-label="Selected fixture address"
						keyboardLabel="Fixture address"
						value={selectedValue}
						onChange={(event) => {
							const address = parsePatchAddress(event.target.value);
							if (address)
								updateBatchPatch(
									controller,
									selectedIndex,
									address.universe,
									address.address,
								);
						}}
					/>
					<p>
						Drag any blue footprint to detach and position that fixture. Arrange
						as block joins them again from the first fixture.
					</p>
					{ui.batchPatches.map((patch, index) => (
						<DmxAddressField
							key={`${index}-${patch}`}
							label={fixtureLabel(controller, index)}
							value={patch}
							details={`${footprint} slots`}
							onOpen={() => setSelectedKey(String(index))}
						/>
					))}
				</div>
				<UniverseMap
					fixtures={all}
					universe={selectedAddress?.universe ?? shownUniverse}
					proposed={selectedAddress?.address ?? 0}
					footprint={footprint}
					proposedLabel={fixtureLabel(controller, selectedIndex)}
					proposals={proposals}
					selectedProposal={selectedKey}
					onSelectedProposal={setSelectedKey}
					onAddress={(address) =>
						updateBatchPatch(controller, selectedIndex, shownUniverse, address)
					}
					onProposalAddress={(key, address) =>
						updateBatchPatch(controller, Number(key), shownUniverse, address)
					}
					onUniverse={(universe) =>
						changePlacementUniverse(controller, universe)
					}
				/>
			</div>
		</section>
	);
}

function fixtureLabel(controller: PatchController, index: number) {
	const { definition } = controller.data;
	const start = parseFixtureNumber(controller.ui.draft.fixtureNumber) ?? 1;
	return `Fixture ${start + index} · ${incrementFixtureName(
		controller.ui.draft.name || definition?.name || "Fixture",
		index,
	)}`;
}
