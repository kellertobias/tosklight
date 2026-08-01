import { createContext, type PropsWithChildren, useContext } from "react";
import type { DynamicsApiClient } from "../../api/client/dynamics";
import type { LightClientRuntime } from "../../api/client/runtime";
import type { ShowObjectsApiClient } from "../../api/client/showObjects";

export interface DynamicsActions {
	dynamics: DynamicsApiClient;
	events?: Pick<LightClientRuntime, "onEvent">;
	showObjects: Pick<
		ShowObjectsApiClient,
		| "object"
		| "createDynamic"
		| "moveDynamic"
		| "copyDynamic"
		| "deleteDynamic"
		| "updateDynamic"
	>;
}

const DynamicsActionsContext = createContext<DynamicsActions | null>(null);

export function DynamicsActionsProvider({
	children,
	actions,
}: PropsWithChildren<{ actions: DynamicsActions }>) {
	return (
		<DynamicsActionsContext.Provider value={actions}>
			{children}
		</DynamicsActionsContext.Provider>
	);
}

/** Authenticated Dynamics actions, or null for isolated component rendering. */
export function useDynamicsActions(): DynamicsActions | null {
	return useContext(DynamicsActionsContext);
}
