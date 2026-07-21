import { createContext, type PropsWithChildren, useContext } from "react";
import { FilesProvider } from "../features/files/FilesContext";
import { ScreensProvider } from "../features/screens/ScreensContext";
import { SelectiveImportProvider } from "../features/selectiveImport/SelectiveImportContext";
import { composeServerContextValue } from "../features/server/composeServerContextValue";
import type { ServerContextValue } from "../features/server/ServerContextValue";
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
import { ServerProgrammingProviders } from "./ServerProgrammingProviders";
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

const ServerContext = createContext<ServerContextValue | null>(null);

export function ServerProvider({
	children,
	sessionRole = "primary",
}: PropsWithChildren<{ sessionRole?: SessionRole }>) {
	const state = useServerState();
	useServerPolling(state);
	const loadShowObjects = useShowObjects(state);
	const refresh = useServerRefresh(state, loadShowObjects);
	const sessionHandoff = useSessionHandoff();
	useServerConnection(state, loadShowObjects, sessionRole, sessionHandoff);
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
	const value = composeServerContextValue(model);
	const fileSource = {
		status: value.status,
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
			value.configuration?.file_manager_system_picker_fallback ?? false,
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
	const selectiveImportSource = {
		catalog: state.client.selectiveImportCatalog,
		preview: state.client.previewSelectiveImport,
		apply: state.client.applySelectiveImport,
		refreshCompatibilityState: refresh,
		reportError: state.setError,
	};
	return (
		<ServerContext.Provider value={value}>
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
											<FilesProvider source={fileSource}>
												<ScreensProvider source={screenSource}>
													{children}
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
		</ServerContext.Provider>
	);
}

export function useServer() {
	const context = useContext(ServerContext);
	if (!context) throw new Error("useServer must be used inside ServerProvider");
	return context;
}
