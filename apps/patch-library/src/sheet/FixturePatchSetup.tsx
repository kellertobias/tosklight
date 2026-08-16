import { useEffect, useRef } from "react";
import type { FixturePatchSetupProps } from "./fixturePatch/controller";
import {
	PatchControllerProvider,
	usePatchController,
} from "./fixturePatch/controller";
import { armEdit } from "./fixturePatch/editSession";
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

export { DmxAddressField } from "./fixturePatch/DmxAddressField";

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

export function FixtureAddFlow(props: FixturePatchSetupProps = {}) {
	return (
		<PatchControllerProvider {...props}>
			<PatchEffects />
			<FixtureBrowser />
			<FixturePlacement />
			<PlacementCloseConfirm />
			<PatchConflictDialog />
		</PatchControllerProvider>
	);
}

export function FixtureAddressFlow({
	fixtureId,
	openRequest,
}: {
	fixtureId: string | null;
	openRequest: number;
}) {
	return (
		<PatchControllerProvider>
			<LinkedFixtureAddress fixtureId={fixtureId} openRequest={openRequest} />
		</PatchControllerProvider>
	);
}

function LinkedFixtureAddress({
	fixtureId,
	openRequest,
}: {
	fixtureId: string | null;
	openRequest: number;
}) {
	const controller = usePatchController();
	const handledRequest = useRef(0);
	useEffect(() => {
		if (!fixtureId || !openRequest || handledRequest.current === openRequest)
			return;
		const fixture = controller.data.all.find(
			(candidate) => candidate.fixture_id === fixtureId,
		);
		if (!fixture) return;
		handledRequest.current = openRequest;
		armEdit(controller, fixture, "address");
	}, [controller, fixtureId, openRequest]);
	return (
		<>
			<PatchEffects />
			<FixtureAddressDialog />
			<PatchConflictDialog />
		</>
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
