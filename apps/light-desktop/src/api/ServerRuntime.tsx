import { type PropsWithChildren, useCallback, useMemo } from "react";
import { AttributeConfigurationActionsProvider } from "../features/attributeConfiguration/AttributeConfigurationActions";
import { CueRecordingProvider } from "../features/cueRecording/CueRecordingProvider";
import { DeskConnectionProvider } from "../features/deskConnection/DeskConnectionContext";
import { DeskLoadingStateProvider } from "../features/deskLoading/DeskLoadingState";
import { DeskStateDiagnosticsProvider } from "../features/deskState/DeskStateDiagnosticsState";
import { DmxDiagnosticsProvider } from "../features/dmxDiagnostics/DmxDiagnosticsContext";
import { DynamicsActionsProvider } from "../features/dynamics/DynamicsActionsContext";
import { FilesProvider } from "../features/files/FilesContext";
import { FixtureLibraryProvider } from "../features/fixtureLibrary/FixtureLibraryContext";
import { GroupRecordingProvider } from "../features/groupRecording/GroupRecordingProvider";
import {
	HighlightActionsProvider,
	HighlightStateProvider,
} from "../features/highlight/HighlightState";
import { MacroActionsProvider } from "../features/macros/MacroActionsContext";
import { MediaServersProvider } from "../features/mediaServers/MediaServersContext";
import { PlaybackTopologyProvider } from "../features/playbackTopology/PlaybackTopologyProvider";
import { PresetRecordingProvider } from "../features/presetRecording/PresetRecordingProvider";
import { ProgrammerActionsProvider } from "../features/programmerActions/ProgrammerActionsContext";
import { RunningRuntimeActionsProvider } from "../features/running/RunningRuntimeActionsContext";
import { SchedulerProvider } from "../features/scheduler/SchedulerContext";
import { useServerSchedulerController } from "../features/scheduler/useServerSchedulerController";
import { ScreensProvider } from "../features/screens/ScreensContext";
import {
	SelectiveImportProvider,
	type SelectiveImportSource,
} from "../features/selectiveImport/SelectiveImportContext";
import { createServerCapabilities } from "../features/server/createServerCapabilities";
import { useCommandLineController } from "../features/server/useCommandLineController";
import { useFileAccess } from "../features/server/useFileAccess";
import { useServerConnection } from "../features/server/useServerConnection";
import { useServerPolling } from "../features/server/useServerPolling";
import { useServerState } from "../features/server/useServerState";
import {
	useServerRefresh,
	useShowObjects,
} from "../features/server/useShowData";
import type { SessionRole } from "../features/session/ownership";
import { useSessionHandoff } from "../features/session/SessionHandoffContext";
import { ShellStatusActionsProvider } from "../features/shellStatus/ShellStatusActionsProvider";
import { ShowLifecycleProvider } from "../features/showLifecycle/ShowLifecycleContext";
import { ShowObjectsViewProvider } from "../features/showObjects/ShowObjectsView";
import { SoundToLightProvider } from "../features/soundToLight/SoundToLightContext";
import { TimecodeActionsProvider } from "../features/timecode/TimecodeActionsContext";
import { VirtualPlaybackZonesProvider } from "../features/virtualPlaybackZones/VirtualPlaybackZonesContext";
import { VisualizerViewProvider } from "../features/visualizerView/VisualizerViewContext";
import type { VisualizerViewPatch } from "./client/visualizerView";
import { supplementalEventSource } from "./runtimeModels";
import { ServerDeskBoundaries } from "./ServerDeskBoundaries";
import { ServerProgrammingProviders } from "./ServerProgrammingProviders";
import { ServerVisualizationRuntimeBoundary } from "./ServerVisualizationRuntimeBoundary";
import type { TimecodeTransportSnapshot } from "./types/timecode";
import { useServerFeatureBoundaries } from "./useServerFeatureBoundaries";

export type {
	CommandChoiceOption,
	PendingCommandChoice,
	StagePosition3d,
	StoredDeskLayout,
	StoredStageLayout,
} from "../features/server/contracts";
export {
	cueOnlyRestoration,
	deskLayoutScopeKey,
} from "../features/server/contracts";

function ServerConnectionOwner({
	children,
	state,
	loadShowObjects,
	sessionRole,
}: PropsWithChildren<{
	state: ReturnType<typeof useServerState>;
	loadShowObjects: ReturnType<typeof useShowObjects>;
	sessionRole: SessionRole;
}>) {
	const sessionHandoff = useSessionHandoff();
	useServerConnection(state, loadShowObjects, sessionRole, sessionHandoff);
	return children;
}

/** Data-shaped provider sources, extracted from ServerRuntime for size. */
function useProviderDataSources(
	state: ReturnType<typeof useServerState>,
	value: ReturnType<typeof createServerCapabilities>,
) {
	const fileSource = {
		status: state.status,
		fileRoots: value.fileRoots,
		fileEntries: value.fileEntries,
		fileMetadata: value.fileMetadata,
		readFileNote: value.readFileNote,
		saveFileNote: value.saveFileNote,
		readTextFile: value.readTextFile,
		saveTextFile: value.saveTextFile,
		fileOperation: value.fileOperation,
		fileContent: value.fileContent,
		fileStreamUrl: value.fileStreamUrl,
		fileThumbnail: value.fileThumbnail,
		claimFileInput: value.claimFileInput,
		releaseFileInput: value.releaseFileInput,
		systemPickerFallback:
			state.configuration?.file_manager_system_picker_fallback ?? false,
	};
	const screenSource = {
		screens: value.screens,
		bootstrap: value.bootstrap,
		session: value.session,
		saveScreen: value.saveScreen,
		deleteScreen: value.deleteScreen,
		setScreenPage: value.setScreenPage,
		updateProgrammerControlSurface: value.updateProgrammerControlSurface,
		updateControlDesk: value.updateControlDesk,
		selectControlDesk: value.selectControlDesk,
		removeClient: value.removeClient,
	};
	const showLifecycle = useShowLifecycleSource(value);
	const deskConnection = useMemo(
		() => ({
			setServerUrl: value.setServerUrl,
			setDeskToken: value.setDeskToken,
			deskLayout: value.deskLayout,
			deskLayoutScope: value.deskLayoutScope,
			saveDeskLayout: value.saveDeskLayout,
		}),
		[
			value.setServerUrl,
			value.setDeskToken,
			value.deskLayout,
			value.deskLayoutScope,
			value.saveDeskLayout,
		],
	);
	const fixtureLibraryState = useMemo(
		() => ({
			fixtureLibrary: value.fixtureLibrary,
			fixtureProfiles: value.fixtureProfiles,
			fixtureProfileWarnings: value.fixtureProfileWarnings,
			patchLayers: value.patchLayers,
			unresolvedMvrFixtures: value.unresolvedMvrFixtures,
			savePatchLayer: value.savePatchLayer,
			saveFixtureProfile: value.saveFixtureProfile,
			deleteFixtureProfile: value.deleteFixtureProfile,
			fixtureProfileRevisions: value.fixtureProfileRevisions,
			saveFixtureProfileSourceGdtf: value.saveFixtureProfileSourceGdtf,
			importFixturePackage: value.importFixturePackage,
			exportFixturePackage: value.exportFixturePackage,
			fixtureSourceMappings: value.fixtureSourceMappings,
			rememberFixtureSourceMapping: value.rememberFixtureSourceMapping,
			gelCatalogs: value.gelCatalogs,
			previewGelCatalogCsvImport: value.previewGelCatalogCsvImport,
			confirmGelCatalogCsvImport: value.confirmGelCatalogCsvImport,
		}),
		[
			value.fixtureLibrary,
			value.fixtureProfiles,
			value.fixtureProfileWarnings,
			value.patchLayers,
			value.unresolvedMvrFixtures,
			value.savePatchLayer,
			value.saveFixtureProfile,
			value.deleteFixtureProfile,
			value.fixtureProfileRevisions,
			value.saveFixtureProfileSourceGdtf,
			value.importFixturePackage,
			value.exportFixturePackage,
			value.fixtureSourceMappings,
			value.rememberFixtureSourceMapping,
			value.gelCatalogs,
			value.previewGelCatalogCsvImport,
			value.confirmGelCatalogCsvImport,
		],
	);
	const mediaServersState = useMemo(
		() => ({
			mediaServers: value.mediaServers,
			mediaPreviewUrls: value.mediaPreviewUrls,
			refreshMediaPreview: value.refreshMediaPreview,
			refreshMediaThumbnails: value.refreshMediaThumbnails,
			inspectMediaServer: value.inspectMediaServer,
			nativeMedia: value.nativeMedia,
			updateNativeMediaText: value.updateNativeMediaText,
			applyMediaLibrarySelection: value.applyMediaLibrarySelection,
			mediaThumbnail: value.mediaThumbnail,
			matter: value.matter,
		}),
		[
			value.mediaServers,
			value.mediaPreviewUrls,
			value.refreshMediaPreview,
			value.refreshMediaThumbnails,
			value.inspectMediaServer,
			value.nativeMedia,
			value.updateNativeMediaText,
			value.applyMediaLibrarySelection,
			value.mediaThumbnail,
			value.matter,
		],
	);
	return {
		fileSource,
		screenSource,
		showLifecycle,
		deskConnection,
		fixtureLibraryState,
		mediaServersState,
	};
}

/** The show library and its lifecycle, extracted from the data sources for size. */
function useShowLifecycleSource(
	value: ReturnType<typeof createServerCapabilities>,
) {
	return useMemo(
		() => ({
			shows: value.shows,
			openShow: value.openShow,
			openCleanDefaultShow: value.openCleanDefaultShow,
			discoveredVisualizers: value.discoveredVisualizers,
			loadFromVisualizer: value.loadFromVisualizer,
			initializeEmptyShow: value.initializeEmptyShow,
			saveShowAs: value.saveShowAs,
			overwriteShow: value.overwriteShow,
			uploadShow: value.uploadShow,
			downloadShow: value.downloadShow,
			listShowRevisions: value.listShowRevisions,
			saveShowRevision: value.saveShowRevision,
			openShowRevision: value.openShowRevision,
			previewMvr: value.previewMvr,
			applyMvr: value.applyMvr,
			previewMvrExport: value.previewMvrExport,
			downloadMvr: value.downloadMvr,
			createUser: value.createUser,
			changeUser: value.changeUser,
			switchUser: value.switchUser,
			shutdownServer: value.shutdownServer,
		}),
		[
			value.shows,
			value.openShow,
			value.openCleanDefaultShow,
			value.discoveredVisualizers,
			value.loadFromVisualizer,
			value.initializeEmptyShow,
			value.saveShowAs,
			value.overwriteShow,
			value.uploadShow,
			value.downloadShow,
			value.listShowRevisions,
			value.saveShowRevision,
			value.openShowRevision,
			value.previewMvr,
			value.applyMvr,
			value.previewMvrExport,
			value.downloadMvr,
			value.createUser,
			value.changeUser,
			value.switchUser,
			value.shutdownServer,
		],
	);
}

/** Action-shaped provider sources, extracted from ServerRuntime for size. */
function useProviderActionSources(
	value: ReturnType<typeof createServerCapabilities>,
) {
	const highlightActions = useMemo(
		() => ({
			highlightAction: value.highlightAction,
			dismissHighlightError: value.dismissHighlightError,
			setPatchPreviewHighlight: value.setPatchPreviewHighlight,
		}),
		[
			value.highlightAction,
			value.dismissHighlightError,
			value.setPatchPreviewHighlight,
		],
	);
	const programmerActions = useMemo(
		() => ({
			undoProgrammer: value.undoProgrammer,
			clearProgrammer: value.clearProgrammer,
			controlFixtureAction: value.controlFixtureAction,
			controlFixtureActions: value.controlFixtureActions,
			generateFixturePresets: value.generateFixturePresets,
			alignSelection: value.alignSelection,
			storePreload: value.storePreload,
		}),
		[
			value.undoProgrammer,
			value.clearProgrammer,
			value.controlFixtureAction,
			value.controlFixtureActions,
			value.generateFixturePresets,
			value.alignSelection,
			value.storePreload,
		],
	);
	const dmxDiagnostics = useMemo(
		() => ({
			readDmx: value.readDmx,
			setDmxOverride: value.setDmxOverride,
			outputRoutes: value.outputRoutes,
			saveOutputRoute: value.saveOutputRoute,
			createOutputRouteRange: value.createOutputRouteRange,
			deleteOutputRoute: value.deleteOutputRoute,
		}),
		[
			value.readDmx,
			value.setDmxOverride,
			value.outputRoutes,
			value.saveOutputRoute,
			value.createOutputRouteRange,
			value.deleteOutputRoute,
		],
	);
	const soundToLightActions = useMemo(
		() => ({
			speedGroup: value.speedGroup,
			updateSpeedGroup: value.updateSpeedGroup,
			observeSpeedGroup: value.observeSpeedGroup,
			speedGroupAction: value.speedGroupAction,
		}),
		[
			value.speedGroup,
			value.updateSpeedGroup,
			value.observeSpeedGroup,
			value.speedGroupAction,
		],
	);
	const shellStatusActions = useMemo(
		() => ({
			dismissError: value.dismissError,
			simulateError: value.simulateError,
			readServerLogs: value.readServerLogs,
		}),
		[value.dismissError, value.simulateError, value.readServerLogs],
	);
	return {
		highlightActions,
		programmerActions,
		dmxDiagnostics,
		soundToLightActions,
		shellStatusActions,
	};
}

function useDynamicsActionSource(state: ReturnType<typeof useServerState>) {
	return useMemo(
		() => ({
			dynamics: state.api.dynamics,
			events: state.api.runtime,
			showObjects: state.api.showObjects,
		}),
		[state.api],
	);
}

/// What the desk sends its connected visualizers, bound to the one desk connection.
function useVisualizerViewActionSource(
	state: ReturnType<typeof useServerState>,
) {
	return useMemo(
		() => ({
			snapshot: () => state.api.visualizerView.snapshot(),
			update: (target: string, patch: VisualizerViewPatch) =>
				state.api.visualizerView.update(target, patch),
			onConnectionChanged: (listener: (connected: boolean) => void) =>
				state.api.runtime.onEvent((event) => {
					if (event.type === "visualizer_connection_changed")
						listener(event.change.connected);
				}),
		}),
		[state.api, state.status],
	);
}

function ServerActionProviderStack({
	children,
	state,
	data,
	actions,
	dynamicsActions,
	visualizerViewActions,
}: PropsWithChildren<{
	state: ReturnType<typeof useServerState>;
	data: ReturnType<typeof useProviderDataSources>;
	actions: ReturnType<typeof useProviderActionSources>;
	dynamicsActions: ReturnType<typeof useDynamicsActionSource>;
	visualizerViewActions: ReturnType<typeof useVisualizerViewActionSource>;
}>) {
	return (
		<HighlightStateProvider store={state.highlightStore}>
			<HighlightActionsProvider actions={actions.highlightActions}>
				<ProgrammerActionsProvider actions={actions.programmerActions}>
					<DynamicsActionsProvider actions={dynamicsActions}>
						<ShellStatusActionsProvider actions={actions.shellStatusActions}>
							<DmxDiagnosticsProvider diagnostics={actions.dmxDiagnostics}>
								<VisualizerViewProvider actions={visualizerViewActions}>
									<ShowLifecycleProvider lifecycle={data.showLifecycle}>
										<DeskConnectionProvider connection={data.deskConnection}>
											<FixtureLibraryProvider
												library={data.fixtureLibraryState}
											>
												<MediaServersProvider media={data.mediaServersState}>
													<SoundToLightProvider
														actions={actions.soundToLightActions}
													>
														{children}
													</SoundToLightProvider>
												</MediaServersProvider>
											</FixtureLibraryProvider>
										</DeskConnectionProvider>
									</ShowLifecycleProvider>
								</VisualizerViewProvider>
							</DmxDiagnosticsProvider>
						</ShellStatusActionsProvider>
					</DynamicsActionsProvider>
				</ProgrammerActionsProvider>
			</HighlightActionsProvider>
		</HighlightStateProvider>
	);
}

function ServerShowProviderStack({
	children,
	state,
	boundaries,
	value,
	data,
	selectiveImportSource,
	sessionRole,
}: PropsWithChildren<{
	state: ReturnType<typeof useServerState>;
	boundaries: ReturnType<typeof useServerFeatureBoundaries>;
	value: ReturnType<typeof createServerCapabilities>;
	data: ReturnType<typeof useProviderDataSources>;
	selectiveImportSource: SelectiveImportSource;
	sessionRole: SessionRole;
}>) {
	const showId = state.bootstrap?.active_show?.id ?? null;
	return (
		<ServerDeskBoundaries state={state} sessionRole={sessionRole}>
			<ServerVisualizationRuntimeBoundary
				state={state}
				sessionRole={sessionRole}
			>
				<ShowObjectsViewProvider
					showId={showId}
					authorityKey={boundaries.showObjectsAuthorityKey}
					store={state.showObjectsStore}
					transport={boundaries.showObjectsTransport}
					loadCollection={boundaries.loadShowObjectCollection}
					loadObject={boundaries.loadShowObjectSnapshot}
					onError={boundaries.reportShowObjectError}
				>
					<PlaybackTopologyProvider
						showId={showId}
						store={state.showObjectsStore}
						transport={boundaries.playbackTopologyTransport}
						loadObject={boundaries.loadShowObject}
						onError={boundaries.reportPlaybackTopologyError}
					>
						<VirtualPlaybackZonesProvider
							authority={boundaries.virtualPlaybackZonesAuthority}
							transport={boundaries.virtualPlaybackZonesTransport}
						>
							<GroupRecordingProvider
								showId={showId}
								store={state.showObjectsStore}
								transport={boundaries.groupRecordingTransport}
								loadGroup={boundaries.loadGroupForRepair}
								onError={boundaries.reportGroupRecordingError}
							>
								<PresetRecordingProvider
									showId={showId}
									store={state.showObjectsStore}
									transport={boundaries.presetRecordingTransport}
									loadPreset={boundaries.loadPresetForRepair}
									onError={boundaries.reportPresetRecordingError}
								>
									<CueRecordingProvider
										showId={showId}
										store={state.showObjectsStore}
										playbackRuntimeStore={state.playbackRuntimeStore}
										transport={boundaries.cueRecordingTransport}
										selectedPlayback={boundaries.selectedCueRecordingPlayback}
										loadObject={boundaries.loadShowObject}
										onError={boundaries.reportCueRecordingError}
									>
										<ServerProgrammingProviders
											state={state}
											boundaries={boundaries}
											value={value}
										>
											<SelectiveImportProvider source={selectiveImportSource}>
												<FilesProvider source={data.fileSource}>
													<ScreensProvider source={data.screenSource}>
														<DeskLoadingStateProvider
															loading={state.deskLoading}
														>
															{children}
														</DeskLoadingStateProvider>
													</ScreensProvider>
												</FilesProvider>
											</SelectiveImportProvider>
										</ServerProgrammingProviders>
									</CueRecordingProvider>
								</PresetRecordingProvider>
							</GroupRecordingProvider>
						</VirtualPlaybackZonesProvider>
					</PlaybackTopologyProvider>
				</ShowObjectsViewProvider>
			</ServerVisualizationRuntimeBoundary>
		</ServerDeskBoundaries>
	);
}

function useRuntimeActionSources(state: ReturnType<typeof useServerState>) {
	const runningRuntimeActions = useMemo(
		() => ({
			macros: state.api.macros,
			timecodes: {
				runtime: (showId: string) => state.api.timecodes.runtime(showId),
				stop: (showId: string, timecodeId: string) =>
					state.api.timecodes.transportAction(showId, timecodeId, {
						type: "stop",
					}),
			},
			showObjects: state.api.showObjects,
			events: supplementalEventSource(state.api.runtime),
		}),
		[state.api],
	);
	const timecodeEvents = useMemo(
		() => ({
			onRuntimeChanged: (
				listener: (snapshot: TimecodeTransportSnapshot) => void,
			) =>
				state.api.runtime.onEvent((event) => {
					if (event.type === "timecode_runtime_changed")
						listener(event.snapshot);
				}),
		}),
		[state.api.runtime],
	);
	return { runningRuntimeActions, timecodeEvents };
}

// @tour frontend-slice:10 Compose focused server capabilities
// The root runtime owns connection state and assembles narrow providers for data, actions, and
// feature stores. Views consume focused capabilities instead of one mutable global server object.
export function ServerRuntime({
	children,
	sessionRole = "primary",
}: PropsWithChildren<{ sessionRole?: SessionRole }>) {
	const state = useServerState();
	useServerPolling(state);
	const loadShowObjects = useShowObjects(state);
	const refresh = useServerRefresh(state, loadShowObjects);
	const commandLine = useCommandLineController(state);
	const fileAccess = useFileAccess(state);
	const boundaries = useServerFeatureBoundaries(state);
	const model = {
		...state,
		sessionRole,
		...commandLine,
		...fileAccess,
		loadShowObjects,
		refresh,
	};
	const value = createServerCapabilities(model);
	const {
		fileSource,
		screenSource,
		showLifecycle,
		deskConnection,
		fixtureLibraryState,
		mediaServersState,
	} = useProviderDataSources(state, value);
	const selectiveImportSource = {
		catalog: state.api.selectiveImport.catalog.bind(state.api.selectiveImport),
		preview: state.api.selectiveImport.preview.bind(state.api.selectiveImport),
		apply: state.api.selectiveImport.apply.bind(state.api.selectiveImport),
		refreshCompatibilityState: refresh,
		reportError: state.setError,
	};
	const {
		highlightActions,
		programmerActions,
		dmxDiagnostics,
		soundToLightActions,
		shellStatusActions,
	} = useProviderActionSources(value);
	const dynamicsActions = useDynamicsActionSource(state);
	const visualizerViewActions = useVisualizerViewActionSource(state);
	const readDeskStateDiagnostics = useCallback(
		() => state.api.runtime.diagnostics(),
		[state.api],
	);
	const refreshAttributeRegistry = useCallback(async () => {
		const next = await state.api.runtime.bootstrap();
		state.setBootstrap((current) =>
			current
				? { ...current, attribute_registry: next.attribute_registry }
				: next,
		);
	}, [state.api, state.setBootstrap]);
	const scheduler = useServerSchedulerController({
		api: state.api,
		showId: state.bootstrap?.active_show?.id ?? null,
		showObjectsStore: state.showObjectsStore,
		runtimeStore: state.schedulerRuntimeStore,
		canWrite: sessionRole === "primary" && state.status === "connected",
		reportError: state.setError,
	});
	const { runningRuntimeActions, timecodeEvents } =
		useRuntimeActionSources(state);
	return (
		<MacroActionsProvider
			actions={{
				macros: state.api.macros,
				showObjects: state.api.showObjects,
				events: supplementalEventSource(state.api.runtime),
			}}
		>
			<TimecodeActionsProvider
				api={state.api.timecodes}
				events={timecodeEvents}
			>
				<RunningRuntimeActionsProvider actions={runningRuntimeActions}>
					<AttributeConfigurationActionsProvider
						client={state.api.attributes}
						showId={state.bootstrap?.active_show?.id ?? null}
						canWrite={sessionRole === "primary" && state.status === "connected"}
						onApplied={refreshAttributeRegistry}
					>
						<SchedulerProvider controller={scheduler}>
							<ServerConnectionOwner
								state={state}
								loadShowObjects={loadShowObjects}
								sessionRole={sessionRole}
							>
								<DeskStateDiagnosticsProvider
									enabled={
										state.status === "connected" && state.session !== null
									}
									readDiagnostics={readDeskStateDiagnostics}
									outputRoutes={state.outputRoutes}
								>
									<ServerActionProviderStack
										state={state}
										data={{
											fileSource,
											screenSource,
											showLifecycle,
											deskConnection,
											fixtureLibraryState,
											mediaServersState,
										}}
										actions={{
											highlightActions,
											programmerActions,
											dmxDiagnostics,
											soundToLightActions,
											shellStatusActions,
										}}
										dynamicsActions={dynamicsActions}
										visualizerViewActions={visualizerViewActions}
									>
										<ServerShowProviderStack
											state={state}
											boundaries={boundaries}
											value={value}
											data={{
												fileSource,
												screenSource,
												showLifecycle,
												deskConnection,
												fixtureLibraryState,
												mediaServersState,
											}}
											selectiveImportSource={selectiveImportSource}
											sessionRole={sessionRole}
										>
											{children}
										</ServerShowProviderStack>
									</ServerActionProviderStack>
								</DeskStateDiagnosticsProvider>
							</ServerConnectionOwner>
						</SchedulerProvider>
					</AttributeConfigurationActionsProvider>
				</RunningRuntimeActionsProvider>
			</TimecodeActionsProvider>
		</MacroActionsProvider>
	);
}
