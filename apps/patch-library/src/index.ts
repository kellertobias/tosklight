/**
 * The ToskLight patch sheet, as a host-agnostic package.
 *
 * A host supplies two things and nothing else: a {@link PatchTransport} that reaches whatever
 * owns the patch, and a {@link PatchHost} describing the surrounding product — fixture library,
 * fixture selection if it has one, and whether inline editing is armed. The lighting desk and the
 * Viz planning application differ in all three and share this one implementation of addressing,
 * conflicts, splits, placement, layers and multi-patch.
 */

export {
	FixturePatchSetup,
	FixtureAddFlow,
	FixtureAddressFlow,
	DmxAddressField,
	batchPatchError,
	compareFixtureIds,
	contiguousBatchPatches,
	definitionModeChannels,
	definitionSplits,
	dmxGridSegments,
	draggedDmxStart,
	effectiveSplitPatches,
	fixtureDisplayId,
	formatFixturePatch,
	formatInstancePatch,
	nextAvailableFixtureNumber,
	parseFixtureNumber,
	parseVirtualFixtureNumber,
	placementBatchCount,
	reconcileModePatchChanges,
	reconcileSplitPatchOwner,
	replaceSelectedSplitPatch,
	splitPatchSetError,
	unpatchFixtureChanges,
	UniverseMap,
	type UniverseMapProposal,
} from "./sheet/FixturePatchSetup";
export type { FixturePatchSetupProps } from "./sheet/fixturePatch/controller";
export {
	fixtureSelectionIds,
	orderedFixtureSelectionIds,
	toggledFixtureSelection,
} from "./sheet/fixturePatch/selection";
export * from "./sheet/fixtureProfileModel";
export * from "./sheet/patchUtils";

export {
	noPatchSelection,
	PatchHostProvider,
	usePatchHost,
	type PatchHost,
	type PatchLibraryHost,
	type PatchSelectionHost,
	type PatchSelectionIntent,
} from "./host";
export {
	noPatchDiagnostics,
	type PatchDiagnostics,
	type PatchMutationSample,
} from "./diagnostics";

export {
	changedPatchFixtureCandidate,
	newPatchFixtureCandidate,
	patchedFixtureCandidate,
	patchedFixtureResults,
	PatchViewProvider,
	usePatch,
	useOptionalPatch,
	usePatchStoreOrNull,
	usePatchView,
	type PatchContextValue,
	type PatchedFixtureResult,
	type PatchFixtureCandidate,
} from "./state/PatchContext";
export { PatchSession, type PatchSessionOptions } from "./state/session";
export { PatchStore, type PatchStoreSnapshot } from "./state/store";
export {
	EMPTY_FIXTURES,
	selectFixturesById,
	selectFixturesForSelection,
	selectPatchedFixtures,
	selectPatchStatus,
	type PatchStatus,
} from "./state/selectors";
export {
	usePatchedFixtures,
	usePatchedFixturesView,
	usePatchFixture,
	usePatchFixturesById,
	usePatchFixturesForSelection,
	usePatchStatus,
	useSelectedPatchedFixtures,
} from "./state/PatchState";
export {
	PATCH_OBJECT_CHANGED_EVENT,
	publishPatchObjectChanged,
} from "./state/externalRepair";
export {
	createPatchDefinitionResolver,
	projectionToPatchedFixture,
} from "./state/model";
export { patchMutation } from "./state/mutationSupport";

/**
 * Fixture and patch data types a host exchanges with the sheet.
 *
 * Exported by name rather than wholesale: `./wire` also carries the desk's REST-era `PatchSnapshot`,
 * which is a different thing from the feature contract of the same name in `./contracts`.
 */
export type {
	FixtureChannel,
	FixtureDefinition,
	FixtureHead,
	FixtureMode,
	FixtureProfile,
	FixtureSplit,
	MultiPatchInstance,
	PatchedFixture,
	PatchLayer,
	SplitPatch,
	VersionedObject,
} from "./wire";

export * from "./contracts";
export * from "./transport";
