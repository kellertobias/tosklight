import { createContext, type PropsWithChildren, useContext } from "react";
import type { ExtensionRuntimeSnapshot } from "../../api/client/deskManagement";

export interface ExtensionRuntimeActions {
	load: () => Promise<ExtensionRuntimeSnapshot>;
	rescan: () => Promise<ExtensionRuntimeSnapshot>;
}

const ExtensionRuntimeActionsContext =
	createContext<ExtensionRuntimeActions | null>(null);

export function ExtensionRuntimeActionsProvider({
	actions,
	children,
}: PropsWithChildren<{ actions: ExtensionRuntimeActions }>) {
	return (
		<ExtensionRuntimeActionsContext.Provider value={actions}>
			{children}
		</ExtensionRuntimeActionsContext.Provider>
	);
}

export function useExtensionRuntimeActions(): ExtensionRuntimeActions | null {
	return useContext(ExtensionRuntimeActionsContext);
}
