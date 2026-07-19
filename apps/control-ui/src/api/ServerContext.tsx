import {
	createContext,
	type PropsWithChildren,
	useCallback,
	useContext,
	useMemo,
	useRef,
} from "react";
import { FilesProvider } from "../features/files/FilesContext";
import { ScreensProvider } from "../features/screens/ScreensContext";
import {
	ShowObjectsViewProvider,
	useShowObjectView,
} from "../features/showObjects/ShowObjectsView";
import type { ShowObject } from "../features/showObjects/contracts";
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
import type { SessionRole } from "../features/session/ownership";
import { configuredServerUrl } from "./LightApiClient";
import { browserDeskBoundaryToken } from "./PatchTransport";
import { WebSocketShowObjectsEventTransport } from "./ShowObjectsEventTransport";

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

function LiveGroupView({ selectedGroupId }: { selectedGroupId: string | null }) {
	useShowObjectView("group", selectedGroupId != null);
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
	const showObjectsTransport = useMemo(
		() =>
			state.session
				? new WebSocketShowObjectsEventTransport({
						baseUrl: configuredServerUrl(),
						sessionToken: state.session.token,
						deskBoundaryToken: browserDeskBoundaryToken(),
					})
				: null,
		[state.session],
	);
	const loadShowObjectCollection = useCallback(
		(showId: string, kind: "group" | "preset") =>
			state.client.objects(showId, kind) as Promise<ShowObject[]>,
		[state.client],
	);
	const lastShowObjectError = useRef<string | null>(null);
	const reportShowObjectError = useCallback(
		(error: Error | null) => {
			if (error) {
				lastShowObjectError.current = error.message;
				state.setError(error.message);
				return;
			}
			state.setError((current) =>
				current === lastShowObjectError.current ? null : current,
			);
			lastShowObjectError.current = null;
		},
		[state.setError],
	);
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
	return (
		<ServerContext.Provider value={value}>
			<ShowObjectsViewProvider
				showId={state.bootstrap?.active_show?.id ?? null}
				store={state.showObjectsStore}
				transport={showObjectsTransport}
				loadCollection={loadShowObjectCollection}
				onError={reportShowObjectError}
			>
				<LiveGroupView selectedGroupId={state.selectedGroupId} />
				<FilesProvider source={fileSource}>
					<ScreensProvider source={screenSource}>{children}</ScreensProvider>
				</FilesProvider>
			</ShowObjectsViewProvider>
		</ServerContext.Provider>
	);
}

export function useServer() {
	const context = useContext(ServerContext);
	if (!context) throw new Error("useServer must be used inside ServerProvider");
	return context;
}
