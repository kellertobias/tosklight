import { createContext, type PropsWithChildren, useContext } from "react";

/**
 * Scoped desk-shell diagnostics: dismissing the surfaced server error and the debug-only
 * error simulation / server event log reads, so diagnostic surfaces stay off the broad
 * server-context path.
 */
export interface ShellStatusActions {
	dismissError: () => void;
	simulateError: (message: string | null) => void;
	readServerLogs: (after?: number) => Promise<
		Array<{ revision: number; kind: string; payload: unknown }>
	>;
}

const ShellStatusActionsContext = createContext<ShellStatusActions | null>(
	null,
);

export function ShellStatusActionsProvider({
	children,
	actions,
}: PropsWithChildren<{ actions: ShellStatusActions }>) {
	return (
		<ShellStatusActionsContext.Provider value={actions}>
			{children}
		</ShellStatusActionsContext.Provider>
	);
}

/** Shell diagnostics actions, or null outside a mounted desk boundary. */
export function useShellStatusActions(): ShellStatusActions | null {
	return useContext(ShellStatusActionsContext);
}
