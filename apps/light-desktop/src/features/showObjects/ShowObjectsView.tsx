import {
	createContext,
	type PropsWithChildren,
	useCallback,
	useContext,
	useEffect,
	useLayoutEffect,
	useMemo,
} from "react";
import { frontendPerformanceDiagnostics } from "../frontendWarmup/diagnostics";
import type { ShowObjectKind } from "./contracts";
import { ShowObjectsStateProvider } from "./ShowObjectsState";
import {
	type ShowObjectCollectionLoader,
	type ShowObjectLoader,
	ShowObjectsSession,
} from "./session";
import type { ShowObjectsStore } from "./store";
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

export interface ShowObjectsAuthority {
	store: ShowObjectsStore;
	activate(kind: ShowObjectKind, objectId?: string): () => void;
	activateKinds(kinds: readonly ShowObjectKind[]): () => void;
}

const ShowObjectsAuthorityContext = createContext<ShowObjectsAuthority | null>(
	null,
);

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
	const measuredLoadCollection = useCallback<ShowObjectCollectionLoader>(
		async (requestedShowId, kind) => {
			const finish = frontendPerformanceDiagnostics.beginSnapshotRequest(
				`show-object:${kind}`,
			);
			try {
				const result = await loadCollection(requestedShowId, kind);
				finish(result);
				return result;
			} catch (error) {
				finish(undefined, error);
				throw error;
			}
		},
		[loadCollection],
	);
	const measuredLoadObject = useCallback<ShowObjectLoader>(
		async (requestedShowId, kind, objectId) => {
			const finish = frontendPerformanceDiagnostics.beginSnapshotRequest(
				`show-object:${kind}:${objectId}`,
			);
			try {
				const result = await loadObject(requestedShowId, kind, objectId);
				finish(result);
				return result;
			} catch (error) {
				finish(undefined, error);
				throw error;
			}
		},
		[loadObject],
	);
	const session = useMemo(
		() =>
			showId
				? new ShowObjectsSession({
						showId,
						authorityKey,
						store,
						transport,
						loadCollection: measuredLoadCollection,
						loadObject: measuredLoadObject,
						onError,
					})
				: null,
		[
			authorityKey,
			measuredLoadCollection,
			measuredLoadObject,
			onError,
			showId,
			store,
			transport,
		],
	);
	useLayoutEffect(() => () => session?.stop(), [session]);
	const authority = useMemo<ShowObjectsAuthority | null>(
		() =>
			session
				? {
						store,
						activate: (kind, objectId) => session.activate(kind, objectId),
						activateKinds: (kinds) => session.activateKinds(kinds),
					}
				: null,
		[session, store],
	);
	return (
		<ShowObjectsStateProvider store={store}>
			<ShowObjectsAuthorityContext.Provider value={authority}>
				<ShowObjectsViewContext.Provider value={session}>
					{children}
				</ShowObjectsViewContext.Provider>
			</ShowObjectsAuthorityContext.Provider>
		</ShowObjectsStateProvider>
	);
}

export function useShowObjectsAuthority() {
	return useContext(ShowObjectsAuthorityContext);
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

/** Activates one complete multi-kind view scope before opening its event stream. */
export function useShowObjectKindsView(
	kinds: readonly ShowObjectKind[],
	enabled = true,
) {
	const scopeKey = kinds.join("|");
	const session = useContext(ShowObjectsViewContext);
	useEffect(() => {
		if (!session || !enabled || !scopeKey) return;
		return session.activateKinds(scopeKey.split("|") as ShowObjectKind[]);
	}, [enabled, scopeKey, session]);
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
