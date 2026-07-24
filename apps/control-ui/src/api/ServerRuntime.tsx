import { type PropsWithChildren, useMemo } from "react";
import {
	HighlightActionsProvider,
	HighlightStateProvider,
} from "../features/highlight/HighlightState";
import { ProgrammerActionsProvider } from "../features/programmerActions/ProgrammerActionsContext";
import { ShellStatusActionsProvider } from "../features/shellStatus/ShellStatusActionsProvider";
import { DmxDiagnosticsProvider } from "../features/dmxDiagnostics/DmxDiagnosticsContext";
import { DeskConnectionProvider } from "../features/deskConnection/DeskConnectionContext";
import { FixtureLibraryProvider } from "../features/fixtureLibrary/FixtureLibraryContext";
import { ShowLifecycleProvider } from "../features/showLifecycle/ShowLifecycleContext";
import { MediaServersProvider } from "../features/mediaServers/MediaServersContext";
import { SoundToLightProvider } from "../features/soundToLight/SoundToLightContext";
import { FilesProvider } from "../features/files/FilesContext";
import { ScreensProvider } from "../features/screens/ScreensContext";
import { SelectiveImportProvider } from "../features/selectiveImport/SelectiveImportContext";
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
import { PresetRecordingProvider } from "../features/presetRecording/PresetRecordingProvider";
import { GroupRecordingProvider } from "../features/groupRecording/GroupRecordingProvider";
import { CueRecordingProvider } from "../features/cueRecording/CueRecordingProvider";
import { PlaybackTopologyProvider } from "../features/playbackTopology/PlaybackTopologyProvider";
import type { SessionRole } from "../features/session/ownership";
import { useSessionHandoff } from "../features/session/SessionHandoffContext";
import { ShowObjectsViewProvider } from "../features/showObjects/ShowObjectsView";
import { VirtualPlaybackZonesProvider } from "../features/virtualPlaybackZones/VirtualPlaybackZonesContext";
import { ServerDeskBoundaries } from "./ServerDeskBoundaries";
import { ServerProgrammingProviders } from "./ServerProgrammingProviders";
import { ServerVisualizationRuntimeBoundary } from "./ServerVisualizationRuntimeBoundary";
import { useServerFeatureBoundaries } from "./useServerFeatureBoundaries";
import { DeskLoadingStateProvider } from "../features/deskLoading/DeskLoadingState";

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
	refresh: ReturnType<typeof useServerRefresh>,
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
		updateControlDesk: value.updateControlDesk,
		selectControlDesk: value.selectControlDesk,
		removeClient: value.removeClient,
	};
	const showLifecycle = useMemo(
		() => ({
			shows: value.shows,
			openShow: value.openShow,
			openCleanDefaultShow: value.openCleanDefaultShow,
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
		],
	);
	const mediaServersState = useMemo(
		() => ({
			mediaServers: value.mediaServers,
			mediaPreviewUrls: value.mediaPreviewUrls,
			refreshMediaPreview: value.refreshMediaPreview,
			refreshMediaThumbnails: value.refreshMediaThumbnails,
			matter: value.matter,
		}),
		[
			value.mediaServers,
			value.mediaPreviewUrls,
			value.refreshMediaPreview,
			value.refreshMediaThumbnails,
			value.matter,
		],
	);
	return {
		fileSource, screenSource, showLifecycle,
		deskConnection, fixtureLibraryState, mediaServersState,
	};
}

/** Action-shaped provider sources, extracted from ServerRuntime for size. */
function useProviderActionSources(value: ReturnType<typeof createServerCapabilities>) {
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
			generateFixturePresets: value.generateFixturePresets,
			alignSelection: value.alignSelection,
			storePreload: value.storePreload,
		}),
		[
			value.undoProgrammer,
			value.clearProgrammer,
			value.controlFixtureAction,
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
			deleteOutputRoute: value.deleteOutputRoute,
		}),
		[
			value.readDmx,
			value.setDmxOverride,
			value.outputRoutes,
			value.saveOutputRoute,
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
	return { highlightActions, programmerActions, dmxDiagnostics, soundToLightActions, shellStatusActions };
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
		fileSource, screenSource, showLifecycle,
		deskConnection, fixtureLibraryState, mediaServersState,
	} = useProviderDataSources(state, value, refresh);
	const selectiveImportSource = {
		catalog: state.api.selectiveImport.catalog.bind(state.api.selectiveImport),
		preview: state.api.selectiveImport.preview.bind(state.api.selectiveImport),
		apply: state.api.selectiveImport.apply.bind(state.api.selectiveImport),
		refreshCompatibilityState: refresh,
		reportError: state.setError,
	};
	const { highlightActions, programmerActions, dmxDiagnostics, soundToLightActions, shellStatusActions } =
		useProviderActionSources(value);
	return (
		<ServerConnectionOwner
			state={state}
			loadShowObjects={loadShowObjects}
			sessionRole={sessionRole}
		>
		<HighlightStateProvider store={state.highlightStore}>
			<HighlightActionsProvider actions={highlightActions}>
			<ProgrammerActionsProvider actions={programmerActions}>
			<ShellStatusActionsProvider actions={shellStatusActions}>
			<DmxDiagnosticsProvider diagnostics={dmxDiagnostics}>
			<ShowLifecycleProvider lifecycle={showLifecycle}>
			<DeskConnectionProvider connection={deskConnection}>
			<FixtureLibraryProvider library={fixtureLibraryState}>
			<MediaServersProvider media={mediaServersState}>
			<SoundToLightProvider actions={soundToLightActions}>
			<ServerDeskBoundaries state={state}>
			<ServerVisualizationRuntimeBoundary state={state}>
				<ShowObjectsViewProvider
					showId={state.bootstrap?.active_show?.id ?? null}
					authorityKey={boundaries.showObjectsAuthorityKey}
					store={state.showObjectsStore}
					transport={boundaries.showObjectsTransport}
					loadCollection={boundaries.loadShowObjectCollection}
					loadObject={boundaries.loadShowObjectSnapshot}
					onError={boundaries.reportShowObjectError}
				>
					<PlaybackTopologyProvider
						showId={state.bootstrap?.active_show?.id ?? null}
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
								showId={state.bootstrap?.active_show?.id ?? null}
								store={state.showObjectsStore}
								transport={boundaries.groupRecordingTransport}
								loadGroup={boundaries.loadGroupForRepair}
								onError={boundaries.reportGroupRecordingError}
							>
								<PresetRecordingProvider
									showId={state.bootstrap?.active_show?.id ?? null}
									store={state.showObjectsStore}
									transport={boundaries.presetRecordingTransport}
									loadPreset={boundaries.loadPresetForRepair}
									onError={boundaries.reportPresetRecordingError}
								>
									<CueRecordingProvider
										showId={state.bootstrap?.active_show?.id ?? null}
										store={state.showObjectsStore}
										playbackRuntimeStore={state.playbackRuntimeStore}
										transport={boundaries.cueRecordingTransport}
										selectedPlayback={
											boundaries.selectedCueRecordingPlayback
										}
										loadObject={boundaries.loadShowObject}
										onError={boundaries.reportCueRecordingError}
									>
										<ServerProgrammingProviders
											state={state}
											boundaries={boundaries}
											value={value}
										>
											<SelectiveImportProvider source={selectiveImportSource}>
												<FilesProvider source={fileSource}>
													<ScreensProvider source={screenSource}>
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
			</SoundToLightProvider>
			</MediaServersProvider>
			</FixtureLibraryProvider>
			</DeskConnectionProvider>
			</ShowLifecycleProvider>
			</DmxDiagnosticsProvider>
			</ShellStatusActionsProvider>
			</ProgrammerActionsProvider>
			</HighlightActionsProvider>
		</HighlightStateProvider>
		</ServerConnectionOwner>
	);
}
