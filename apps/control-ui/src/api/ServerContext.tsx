import { createContext, type PropsWithChildren, useContext } from "react";
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

export function ServerProvider({ children }: PropsWithChildren) {
	const state = useServerState();
	useServerPolling(state);
	const loadShowObjects = useShowObjects(state);
	const refresh = useServerRefresh(state, loadShowObjects);
	useServerConnection(state, loadShowObjects);
	const commandLine = useCommandLineController(state);
	const files = useFileAccess(state);
	const model = {
		...state,
		...commandLine,
		...files,
		loadShowObjects,
		refresh,
	};
	const value = composeServerContextValue(model);
	return (
		<ServerContext.Provider value={value}>{children}</ServerContext.Provider>
	);
}

export function useServer() {
	const context = useContext(ServerContext);
	if (!context) throw new Error("useServer must be used inside ServerProvider");
	return context;
}
