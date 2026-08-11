import { createContext, type PropsWithChildren, useContext } from "react";
import type { MacrosApiClient } from "../../api/client/macros";
import type { ShowObjectsApiClient } from "../../api/client/showObjects";
import type { SupplementalEventSource } from "../../api/runtimeModels";

export interface MacroActions {
	macros: MacrosApiClient;
	showObjects: Pick<ShowObjectsApiClient, "objects">;
	events?: SupplementalEventSource;
}

const MacroActionsContext = createContext<MacroActions | null>(null);

export function MacroActionsProvider({
	children,
	actions,
}: PropsWithChildren<{ actions: MacroActions }>) {
	return (
		<MacroActionsContext.Provider value={actions}>
			{children}
		</MacroActionsContext.Provider>
	);
}

export function useMacroActions(): MacroActions | null {
	return useContext(MacroActionsContext);
}
