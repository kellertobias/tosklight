import { createContext, type PropsWithChildren, useContext } from "react";
import type { VersionedObject } from "../../api/types";
import type { StoredDeskLayout } from "../server/contracts";

/**
 * Scoped desk-connection identity and persisted desk layout: the configured server URL and
 * desk token setters plus the stored per-desk window layout.
 */
export interface DeskConnectionState {
	setServerUrl: (url: string) => void;
	setDeskToken: (token: string) => void;
	deskLayout: VersionedObject<StoredDeskLayout> | null;
	deskLayoutScope: string | null;
	saveDeskLayout: (layout: StoredDeskLayout) => Promise<void>;
}

const DeskConnectionContext = createContext<DeskConnectionState | null>(null);

export function DeskConnectionProvider({
	children,
	connection,
}: PropsWithChildren<{ connection: DeskConnectionState }>) {
	return (
		<DeskConnectionContext.Provider value={connection}>
			{children}
		</DeskConnectionContext.Provider>
	);
}

/** Desk connection state, or null outside a mounted desk boundary. */
export function useDeskConnection(): DeskConnectionState | null {
	return useContext(DeskConnectionContext);
}
