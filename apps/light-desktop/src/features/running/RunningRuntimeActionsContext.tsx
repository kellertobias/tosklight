import { createContext, type PropsWithChildren, useContext } from "react";
import type { MacrosApiClient } from "../../api/client/macros";
import type { TimecodeRunningApi } from "../../api/client/runningRuntime";
import type { ShowObjectsApiClient } from "../../api/client/showObjects";
import type { SupplementalEventSource } from "../../api/runtimeModels";

export interface RunningRuntimeActions {
	macros: Pick<MacrosApiClient, "runtime" | "cancel">;
	timecodes: TimecodeRunningApi;
	showObjects: Pick<ShowObjectsApiClient, "objects">;
	events?: SupplementalEventSource;
}

const RunningRuntimeActionsContext =
	createContext<RunningRuntimeActions | null>(null);

export function RunningRuntimeActionsProvider({
	children,
	actions,
}: PropsWithChildren<{ actions: RunningRuntimeActions }>) {
	return (
		<RunningRuntimeActionsContext.Provider value={actions}>
			{children}
		</RunningRuntimeActionsContext.Provider>
	);
}

export function useRunningRuntimeActions(): RunningRuntimeActions | null {
	return useContext(RunningRuntimeActionsContext);
}
