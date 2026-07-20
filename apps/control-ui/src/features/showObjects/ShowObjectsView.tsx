import {
	createContext,
	type PropsWithChildren,
	useContext,
	useEffect,
	useLayoutEffect,
	useMemo,
} from "react";
import type { ShowObjectKind } from "./contracts";
import {
	ShowObjectsSession,
	type ShowObjectCollectionLoader,
	type ShowObjectLoader,
} from "./session";
import { ShowObjectsStateProvider } from "./ShowObjectsState";
import { ShowObjectsStore } from "./store";
import type { ShowObjectsEventTransport } from "./transport";

interface ShowObjectsViewProviderProps {
	showId: string | null;
	authorityKey?: string;
	store: ShowObjectsStore;
	transport: ShowObjectsEventTransport | null;
	loadCollection: ShowObjectCollectionLoader;
	loadObject: ShowObjectLoader;
	onError?: (error: Error | null) => void;
}

const ShowObjectsViewContext = createContext<ShowObjectsSession | null>(null);

export function ShowObjectsViewProvider({
	children,
	showId,
	authorityKey,
	store,
	transport,
	loadCollection,
	loadObject,
	onError,
}: PropsWithChildren<ShowObjectsViewProviderProps>) {
	useLayoutEffect(() => {
		store.reset(showId, authorityKey);
	}, [authorityKey, showId, store]);
	const session = useMemo(
		() =>
			showId
				? new ShowObjectsSession({
						showId,
						store,
						transport,
						loadCollection,
						loadObject,
						onError,
					})
				: null,
		[
			authorityKey,
			loadCollection,
			loadObject,
			onError,
			showId,
			store,
			transport,
		],
	);
	useLayoutEffect(() => () => session?.stop(), [session]);
	return (
		<ShowObjectsStateProvider store={store}>
			<ShowObjectsViewContext.Provider value={session}>
				{children}
			</ShowObjectsViewContext.Provider>
		</ShowObjectsStateProvider>
	);
}

/** Keeps the smallest show-object event subscription alive for this mounted view. */
export function useShowObjectView(
	kind: ShowObjectKind,
	enabled = true,
	objectId?: string,
) {
	const session = useContext(ShowObjectsViewContext);
	useEffect(() => {
		if (!session || !enabled) return;
		return session.activate(kind, objectId);
	}, [enabled, kind, objectId, session]);
}

/** Owns an exact-object subscription for a selected detail projection. */
export function ShowObjectDetailSubscription({
	kind,
	objectId,
}: {
	kind: ShowObjectKind;
	objectId: string | null;
}) {
	useShowObjectView(kind, objectId != null, objectId ?? undefined);
	return null;
}
