import type { FixturePatchSetupProps } from "./fixturePatch/controller";
import {
	PatchControllerProvider,
	usePatchController,
} from "./fixturePatch/controller";
import { FixtureBrowser } from "./fixturePatch/FixtureBrowser";
import { FixturePlacement } from "./fixturePatch/FixturePlacement";
import { PatchHeader, PatchLayers } from "./fixturePatch/PatchChrome";
import {
	AddLayerDialog,
	DeleteConfirm,
	EditCloseConfirm,
	PatchConflictDialog,
	PlacementCloseConfirm,
} from "./fixturePatch/PatchDialogs";
import {
	FixtureAddressDialog,
	FixtureEditDialog,
	MultipatchAddressDialog,
	MultipatchVectorDialog,
} from "./fixturePatch/PatchEditSurfaces";
import { PatchEffects } from "./fixturePatch/PatchEffects";
import { PatchTable } from "./fixturePatch/PatchTable";

/**
 * The patch sheet.
 *
 * The host composes the patch authority (`PatchViewProvider`) and the host port
 * (`PatchHostProvider`) above this; the sheet itself never reaches for either transport or
 * application state.
 */
export function FixturePatchSetup(props: FixturePatchSetupProps = {}) {
	return (
		<PatchControllerProvider {...props}>
			<FixturePatchLayout />
		</PatchControllerProvider>
	);
}

function FixturePatchLayout() {
	const { ui } = usePatchController();
	return (
		<div
			className={`show-patch-layout ${ui.layerModal === "select" ? "layer-selecting" : ""}`}
		>
			<PatchEffects />
			<PatchHeader />
			<PatchLayers />
			<PatchTable />
			<FixtureBrowser />
			<FixturePlacement />
			<PlacementCloseConfirm />
			<EditCloseConfirm />
			<DeleteConfirm />
			<MultipatchVectorDialog />
			<MultipatchAddressDialog />
			<AddLayerDialog />
			<FixtureEditDialog />
			<FixtureAddressDialog />
			<PatchConflictDialog />
		</div>
	);
}

export {
	batchPatchError,
	compareFixtureIds,
	contiguousBatchPatches,
	fixtureDisplayId,
	nextAvailableFixtureNumber,
	parseFixtureNumber,
	parseVirtualFixtureNumber,
	placementBatchCount,
} from "./fixturePatch/fixtureIds";
export {
	definitionModeChannels,
	definitionSplits,
	effectiveSplitPatches,
	formatFixturePatch,
	formatInstancePatch,
	reconcileModePatchChanges,
	reconcileSplitPatchOwner,
	replaceSelectedSplitPatch,
	splitPatchSetError,
	unpatchFixtureChanges,
} from "./fixturePatch/patchModel";
export {
	dmxGridSegments,
	draggedDmxStart,
	UniverseMap,
	type UniverseMapProposal,
} from "./fixturePatch/UniverseMap";
