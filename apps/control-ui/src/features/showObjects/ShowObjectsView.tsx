import {
	createContext,
	type PropsWithChildren,
	useContext,
	useEffect,
	useMemo,
} from "react";
import type { ShowObjectKind } from "./contracts";
import { ShowObjectsSession, type ShowObjectCollectionLoader } from "./session";
import { ShowObjectsStore } from "./store";
import type { ShowObjectsEventTransport } from "./transport";

interface ShowObjectsViewProviderProps {
	showId: string | null;
	store: ShowObjectsStore;
	transport: ShowObjectsEventTransport | null;
	loadCollection: ShowObjectCollectionLoader;
	onError?: (error: Error | null) => void;
}

const ShowObjectsViewContext = createContext<ShowObjectsSession | null>(null);

export function ShowObjectsViewProvider({
	children,
	showId,
	store,
	transport,
	loadCollection,
	onError,
}: PropsWithChildren<ShowObjectsViewProviderProps>) {
	const session = useMemo(
		() =>
			showId && transport
				? new ShowObjectsSession({
						showId,
						store,
						transport,
						loadCollection,
						onError,
					})
				: null,
		[loadCollection, onError, showId, store, transport],
	);
	useEffect(() => () => session?.stop(), [session]);
	return (
		<ShowObjectsViewContext.Provider value={session}>
			{children}
		</ShowObjectsViewContext.Provider>
	);
}

/** Keeps the smallest show-object event subscription alive for this mounted view. */
export function useShowObjectView(kind: ShowObjectKind, enabled = true) {
	const session = useContext(ShowObjectsViewContext);
	useEffect(() => {
		if (!session || !enabled) return;
		return session.activate(kind);
	}, [enabled, kind, session]);
}
