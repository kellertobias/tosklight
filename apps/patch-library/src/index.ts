/**
 * The ToskLight patch sheet, as a host-agnostic package.
 *
 * A host supplies two things and nothing else: a {@link PatchTransport} that reaches whatever
 * owns the patch, and a {@link PatchHost} describing the surrounding product — fixture library,
 * fixture selection if it has one, and whether inline editing is armed. The lighting desk and the
 * Viz planning application differ in all three and share this one implementation of addressing,
 * conflicts, splits, placement, layers and multi-patch.
 */

export * from "./contracts";
export {
	noPatchDiagnostics,
	type PatchDiagnostics,
	type PatchMutationSample,
} from "./diagnostics";
export {
	noPatchSelection,
	type PatchHost,
	PatchHostProvider,
	type PatchLibraryHost,
	type PatchSelectionHost,
	type PatchSelectionIntent,
	usePatchHost,
} from "./host";
export {
	batchPatchError,
	compareFixtureIds,
	contiguousBatchPatches,
	DmxAddressField,
	definitionModeChannels,
	definitionSplits,
	dmxGridSegments,
	draggedDmxStart,
	effectiveSplitPatches,
	FixtureAddFlow,
	FixtureAddressFlow,
	FixturePatchSetup,
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
	UniverseMap,
	type UniverseMapProposal,
	unpatchFixtureChanges,
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
	PATCH_OBJECT_CHANGED_EVENT,
	publishPatchObjectChanged,
} from "./state/externalRepair";
export {
	createPatchDefinitionResolver,
	projectionToPatchedFixture,
} from "./state/model";
export { patchMutation } from "./state/mutationSupport";
export {
	changedPatchFixtureCandidate,
	newPatchFixtureCandidate,
	type PatchContextValue,
	type PatchedFixtureResult,
	type PatchFixtureCandidate,
	PatchViewProvider,
	patchedFixtureCandidate,
	patchedFixtureResults,
	useOptionalPatch,
	usePatch,
	usePatchStoreOrNull,
	usePatchView,
} from "./state/PatchContext";
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
	EMPTY_FIXTURES,
	type PatchStatus,
	selectFixturesById,
	selectFixturesForSelection,
	selectPatchedFixtures,
	selectPatchStatus,
} from "./state/selectors";
export { PatchSession, type PatchSessionOptions } from "./state/session";
export { PatchStore, type PatchStoreSnapshot } from "./state/store";
export * from "./transport";
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
	FixtureVisibility,
	MultiPatchInstance,
	PatchedFixture,
	PatchLayer,
	SplitPatch,
	VersionedObject,
} from "./wire";
