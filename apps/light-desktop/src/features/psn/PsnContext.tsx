import { createContext, type PropsWithChildren, useContext } from "react";
import type { PsnEdit, PsnSnapshot } from "../../api/client/psn";

/**
 * What the Tracking tab is allowed to do: read everything at once, and edit what changed.
 *
 * Deliberately two calls. Every question the tab asks — which fixtures are 3D Points, whether a
 * tracker is stale, where a bound point ended up — is the desk's answer, so there is nothing here
 * for the surface to compute or to hold an opinion about.
 */
export interface PsnState {
	snapshot: () => Promise<PsnSnapshot>;
	update: (edit: PsnEdit) => Promise<unknown>;
}

const PsnContext = createContext<PsnState | null>(null);

export function PsnProvider({
	children,
	psn,
}: PropsWithChildren<{ psn: PsnState }>) {
	return <PsnContext.Provider value={psn}>{children}</PsnContext.Provider>;
}

/** Tracking desk state, or null outside a mounted desk boundary. */
export function usePsn(): PsnState | null {
	return useContext(PsnContext);
}
