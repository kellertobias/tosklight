import {
	createContext,
	type PropsWithChildren,
	useCallback,
	useContext,
	useEffect,
	useMemo,
	useRef,
	useSyncExternalStore,
} from "react";
import type { PlaybackIdentity, PlaybackProjection } from "./contracts";
import { cueListIdentity, identityKey, playbackIdentity } from "./contracts";
import { legacyPlaybackRuntime } from "./legacy";
import {
	PlaybackRuntimeSession,
	type PlaybackRuntimeSessionOptions,
} from "./session";
import { type PlaybackRuntimeState, PlaybackRuntimeStore } from "./store";
import type { PlaybackEventTransport } from "./transport";

interface PlaybackRuntimeViewProviderProps {
	showId: string | null;
	deskId: string | null;
	store: PlaybackRuntimeStore;
	transport: PlaybackEventTransport | null;
	loadSnapshot: PlaybackRuntimeSessionOptions["loadSnapshot"];
	initialDesk?: { activePage: number; selectedPlayback: number | null } | null;
	onError?: (error: Error | null) => void;
}

const StoreContext = createContext<PlaybackRuntimeStore | null>(null);
const SessionContext = createContext<PlaybackRuntimeSession | null>(null);
const fallbackStore = new PlaybackRuntimeStore();

export function PlaybackRuntimeViewProvider({
	children,
	showId,
	deskId,
	store,
	transport,
	loadSnapshot,
	initialDesk,
	onError,
}: PropsWithChildren<PlaybackRuntimeViewProviderProps>) {
	const session = useMemo(
		() =>
			showId && deskId
				? new PlaybackRuntimeSession({
						showId,
						deskId,
						store,
						transport,
						loadSnapshot,
						onError,
					})
				: null,
		[deskId, loadSnapshot, onError, showId, store, transport],
	);
	useEffect(() => {
		if (!session) store.reset(showId, deskId);
		return () => session?.stop();
	}, [deskId, session, showId, store]);
	useEffect(() => {
		if (initialDesk)
			store.seedDesk(initialDesk.activePage, initialDesk.selectedPlayback);
	}, [initialDesk, store]);
	return (
		<StoreContext.Provider value={store}>
			<SessionContext.Provider value={session}>
				{children}
			</SessionContext.Provider>
		</StoreContext.Provider>
	);
}

export function usePlaybackRuntimeView(
	identities: readonly PlaybackIdentity[],
) {
	const session = useContext(SessionContext);
	const key = identities.map(identityKey).sort().join("|");
	useEffect(() => {
		if (!session || !identities.length) return;
		const releases = identities.map((identity) => session.activate(identity));
		return () => {
			for (const release of releases) release();
		};
		// The canonical key owns identity equality across render-created DTOs.
		// eslint-disable-next-line react-hooks/exhaustive-deps
	}, [key, session]);
}

export function usePlaybackDeskView(enabled = true) {
	const session = useContext(SessionContext);
	useEffect(() => {
		if (!session || !enabled) return;
		return session.activateDesk();
	}, [enabled, session]);
	return usePlaybackSelector(
		useCallback(
			(state: PlaybackRuntimeState) => (enabled ? state.desk : null),
			[enabled],
		),
		Object.is,
	);
}

export function usePlaybackProjection(
	playbackNumber: number | null | undefined,
) {
	const identity = useMemo(
		() => (playbackNumber == null ? [] : [playbackIdentity(playbackNumber)]),
		[playbackNumber],
	);
	usePlaybackRuntimeView(identity);
	return usePlaybackSelector(
		useCallback(
			(state: PlaybackRuntimeState) =>
				playbackNumber == null
					? undefined
					: state.projections
							.get(`playback:${playbackNumber}`)
							?.find((item) => item.playback_number === playbackNumber),
			[playbackNumber],
		),
		Object.is,
	);
}

export function usePlaybackRuntime(playbackNumber: number | null | undefined) {
	return legacyPlaybackRuntime(usePlaybackProjection(playbackNumber));
}

export function useCueListRuntime(
	cueListId: string | null | undefined,
	playbackNumber?: number | null,
) {
	const identities = useMemo(
		() => (cueListId ? [cueListIdentity(cueListId)] : []),
		[cueListId],
	);
	usePlaybackRuntimeView(identities);
	const projection = usePlaybackSelector(
		useCallback(
			(state: PlaybackRuntimeState) => {
				const candidates = cueListId
					? state.projections.get(`cuelist:${cueListId}`)
					: undefined;
				return (
					candidates?.find(
						(item) => item.playback_number === (playbackNumber ?? null),
					) ?? candidates?.[0]
				);
			},
			[cueListId, playbackNumber],
		),
		Object.is,
	);
	return legacyPlaybackRuntime(projection);
}

export function usePlaybackProjectionMap(playbackNumbers: readonly number[]) {
	const canonical = [...new Set(playbackNumbers)].sort((a, b) => a - b);
	const key = canonical.join(",");
	const identities = useMemo(
		() => canonical.map(playbackIdentity),
		// The canonical numeric key owns array equality.
		// eslint-disable-next-line react-hooks/exhaustive-deps
		[key],
	);
	usePlaybackRuntimeView(identities);
	return usePlaybackSelector(
		useCallback(
			(state: PlaybackRuntimeState) =>
				new Map(
					canonical.map((number) => [
						number,
						state.projections
							.get(`playback:${number}`)
							?.find((item) => item.playback_number === number),
					]),
				),
			// The same canonical key denotes the selected number set.
			// eslint-disable-next-line react-hooks/exhaustive-deps
			[key],
		),
		equalProjectionMap,
	);
}

export function usePlaybackRuntimeStatus() {
	return usePlaybackSelector(selectStatus, equalStatus);
}

function usePlaybackSelector<T>(
	selector: (state: PlaybackRuntimeState) => T,
	equal: (left: T, right: T) => boolean,
) {
	const store = useContext(StoreContext) ?? fallbackStore;
	const cache = useRef<{ state: PlaybackRuntimeState | null; value?: T }>({
		state: null,
	});
	const getSelection = useCallback(() => {
		const state = store.getSnapshot();
		if (cache.current.state === state) return cache.current.value as T;
		const value = selector(state);
		if (cache.current.state && equal(cache.current.value as T, value)) {
			cache.current.state = state;
			return cache.current.value as T;
		}
		cache.current = { state, value };
		return value;
	}, [equal, selector, store]);
	return useSyncExternalStore(store.subscribe, getSelection, getSelection);
}

function equalProjectionMap(
	left: ReadonlyMap<number, PlaybackProjection | undefined>,
	right: ReadonlyMap<number, PlaybackProjection | undefined>,
) {
	return (
		left.size === right.size &&
		[...left].every(([key, value]) => right.get(key) === value)
	);
}

function selectStatus(state: PlaybackRuntimeState) {
	return { status: state.status, error: state.error };
}

function equalStatus(
	left: ReturnType<typeof selectStatus>,
	right: ReturnType<typeof selectStatus>,
) {
	return left.status === right.status && left.error === right.error;
}
