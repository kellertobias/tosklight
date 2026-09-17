import { createContext, type PropsWithChildren, useContext } from "react";
import type { SessionRole } from "../session/ownership";

/** How this window joins its desk, and a way to try again right now. */
export interface ConnectionRetryState {
	role: SessionRole;
	retry: () => void;
}

const ConnectionRetryContext = createContext<ConnectionRetryState | null>(null);

export function ConnectionRetryProvider({
	children,
	value,
}: PropsWithChildren<{ value: ConnectionRetryState }>) {
	return (
		<ConnectionRetryContext.Provider value={value}>
			{children}
		</ConnectionRetryContext.Provider>
	);
}

/** The connection retry of the mounted server runtime, or null outside one. */
export function useConnectionRetry(): ConnectionRetryState | null {
	return useContext(ConnectionRetryContext);
}
