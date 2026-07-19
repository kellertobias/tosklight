import { createContext, type PropsWithChildren, useContext } from "react";
import { FilesProvider } from "../features/files/FilesContext";
import { PlaybackRuntimeViewProvider } from "../features/playbackRuntime/PlaybackRuntimeView";
import { ProgrammingInteractionViewProvider } from "../features/programmingInteraction/ProgrammingInteractionView";
import { SelectiveImportProvider } from "../features/selectiveImport/SelectiveImportContext";
import { ScreensProvider } from "../features/screens/ScreensContext";
import { composeServerContextValue } from "../features/server/composeServerContextValue";
import type { ServerContextValue } from "../features/server/ServerContextValue";
import { useCommandLineController } from "../features/server/useCommandLineController";
import { useFileAccess } from "../features/server/useFileAccess";
import { useSelectedGroupMembership } from "../features/server/useSelectedGroupMembership";
import { useServerConnection } from "../features/server/useServerConnection";
import { useServerPolling } from "../features/server/useServerPolling";
import { useServerState } from "../features/server/useServerState";
import {
	useServerRefresh,
	useShowObjects,
} from "../features/server/useShowData";
import { useGroups } from "../features/server/useShowObjectsState";
import type { SessionRole } from "../features/session/ownership";
import {
	ShowObjectDetailSubscription,
	ShowObjectsViewProvider,
} from "../features/showObjects/ShowObjectsView";
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

function SelectedGroupMembershipSync({
	playbacks,
	selectedGroupId,
	setSelectedGroupId,
	setSelectedFixtures,
}: Pick<
	ReturnType<typeof useServerState>,
	"playbacks" | "selectedGroupId" | "setSelectedGroupId" | "setSelectedFixtures"
>) {
	const groups = useGroups(playbacks);
	useSelectedGroupMembership(
		groups,
		selectedGroupId,
		setSelectedGroupId,
		setSelectedFixtures,
	);
	return null;
}

export function ServerProvider({
	children,
	sessionRole = "primary",
}: PropsWithChildren<{ sessionRole?: SessionRole }>) {
	const state = useServerState();
	useServerPolling(state);
	const loadShowObjects = useShowObjects(state);
	const refresh = useServerRefresh(state, loadShowObjects);
	useServerConnection(state, loadShowObjects, sessionRole);
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
		commandLine: value.commandLine,
		resetCommandLine: value.resetCommandLine,
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
		playbacks: value.playbacks,
		saveScreen: value.saveScreen,
		deleteScreen: value.deleteScreen,
		setScreenPage: value.setScreenPage,
		savePlaybackPage: value.savePlaybackPage,
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
				store={state.showObjectsStore}
				transport={boundaries.showObjectsTransport}
				loadCollection={boundaries.loadShowObjectCollection}
				loadObject={boundaries.loadShowObject}
				onError={boundaries.reportShowObjectError}
			>
				<PlaybackRuntimeViewProvider
					showId={state.bootstrap?.active_show?.id ?? null}
					deskId={state.session?.desk.id ?? null}
					store={state.playbackRuntimeStore}
					transport={boundaries.playbackTransport}
					loadSnapshot={boundaries.loadPlaybackSnapshot}
					initialDesk={
						state.playbacks
							? {
									activePage: state.playbacks.active_page,
									selectedPlayback: state.playbacks.selected_playback ?? null,
								}
							: null
					}
					onError={boundaries.reportPlaybackError}
				>
					<ProgrammingInteractionViewProvider
						showId={state.bootstrap?.active_show?.id ?? null}
						deskId={state.session?.desk.id ?? null}
						store={state.programmingInteractionStore}
						transport={boundaries.programmingTransport}
						loadSnapshot={boundaries.loadProgrammingInteractionSnapshot}
						onError={boundaries.reportProgrammingError}
					>
						<SelectedGroupMembershipSync
							playbacks={state.playbacks}
							selectedGroupId={state.selectedGroupId}
							setSelectedGroupId={state.setSelectedGroupId}
							setSelectedFixtures={state.setSelectedFixtures}
						/>
						<ShowObjectDetailSubscription
							kind="group"
							objectId={state.selectedGroupId}
						/>
						<SelectiveImportProvider source={selectiveImportSource}>
							<FilesProvider source={fileSource}>
								<ScreensProvider source={screenSource}>
									{children}
								</ScreensProvider>
							</FilesProvider>
						</SelectiveImportProvider>
					</ProgrammingInteractionViewProvider>
				</PlaybackRuntimeViewProvider>
			</ShowObjectsViewProvider>
		</ServerContext.Provider>
	);
}

export function useServer() {
	const context = useContext(ServerContext);
	if (!context) throw new Error("useServer must be used inside ServerProvider");
	return context;
}
