import {
	createContext,
	type PropsWithChildren,
	useCallback,
	useContext,
	useRef,
	useSyncExternalStore,
} from "react";
import type {
	BootstrapSnapshot,
	OutputHealth,
	SessionResponse,
	ShowEntry,
} from "../../api/types";
import {
	equalActiveShow,
	selectActiveShow,
	selectActiveShowId,
	selectActiveTimecode,
	selectActiveShowError,
	selectAttributeRegistry,
	selectBootstrap,
	selectBootstrapReady,
	selectFrameRateHz,
	selectHardwareConnected,
	selectOutputHealth,
	selectSession,
} from "./selectors";
import {
	type DeskSnapshot,
	DeskSnapshotStore,
	EMPTY_DESK_SNAPSHOT,
} from "./store";

const DeskSnapshotStoreContext = createContext<DeskSnapshotStore | null>(null);

export function DeskSnapshotStateProvider({
	children,
	store,
}: PropsWithChildren<{ store: DeskSnapshotStore }>) {
	return (
		<DeskSnapshotStoreContext.Provider value={store}>
			{children}
		</DeskSnapshotStoreContext.Provider>
	);
}

/** The active show entry, or null while no show is open or the bootstrap is unknown. */
export function useActiveShow(): ShowEntry | null {
	return useDeskSnapshotSelector(selectActiveShow, equalActiveShow);
}

/** The active show id, or null while no show is open or the bootstrap is unknown. */
export function useActiveShowId(): string | null {
	return useDeskSnapshotSelector(selectActiveShowId, Object.is);
}

/** Whether hardware controls are attached to this desk. */
export function useHardwareConnected(): boolean {
	return useDeskSnapshotSelector(selectHardwareConnected, Object.is);
}

/** The configured output frame rate, or null while the bootstrap is unknown. */
export function useFrameRateHz(): number | null {
	return useDeskSnapshotSelector(selectFrameRateHz, Object.is);
}

/** The active timecode display value, or null when no timecode source is active. */
export function useActiveTimecode(): string | null {
	return useDeskSnapshotSelector(selectActiveTimecode, Object.is);
}

/** Output health counters, or null while the bootstrap is unknown. */
export function useOutputHealth(): OutputHealth | null {
	return useDeskSnapshotSelector(selectOutputHealth, Object.is);
}

/** The active-show load error, or null while none is surfaced. */
export function useActiveShowError(): string | null {
	return useDeskSnapshotSelector(selectActiveShowError, Object.is);
}

/** The desk attribute registry, or null while the bootstrap is unknown. */
export function useAttributeRegistry() {
	return useDeskSnapshotSelector(selectAttributeRegistry, Object.is);
}

/** Whether the desk bootstrap has been loaded for this connection. */
export function useBootstrapReady(): boolean {
	return useDeskSnapshotSelector(selectBootstrapReady, Object.is);
}

/**
 * The whole desk bootstrap, for surfaces that genuinely render many of its sections.
 *
 * Prefer a scalar hook: a whole-bootstrap reader rerenders on every bootstrap poll.
 */
export function useBootstrapSnapshot(): BootstrapSnapshot | null {
	return useDeskSnapshotSelector(selectBootstrap, Object.is);
}

/**
 * The desk session, for surfaces that render session identity (desk, user, layout).
 *
 * Action paths keep their own transports; this hook is read-only.
 */
export function useSessionSnapshot(): SessionResponse | null {
	return useDeskSnapshotSelector(selectSession, Object.is);
}

/**
 * Equality-cached desk-snapshot projection.
 *
 * A reader outside a mounted desk boundary observes the inert empty snapshot rather than
 * falling back to broad server state.
 */
function useDeskSnapshotSelector<T>(
	selector: (snapshot: DeskSnapshot) => T,
	equal: (left: T, right: T) => boolean,
): T {
	const store = useContext(DeskSnapshotStoreContext);
	const cache = useRef<{
		source: DeskSnapshot | null;
		selection: T | null;
		hasSelection: boolean;
		selector: ((snapshot: DeskSnapshot) => T) | null;
	}>({ source: null, selection: null, hasSelection: false, selector: null });
	const getSelection = useCallback(() => {
		const source = store ? store.getSnapshot() : EMPTY_DESK_SNAPSHOT;
		if (
			cache.current.selector === selector &&
			cache.current.source === source &&
			cache.current.hasSelection
		)
			return cache.current.selection as T;
		const selection = selector(source);
		if (
			cache.current.selector === selector &&
			cache.current.hasSelection &&
			equal(cache.current.selection as T, selection)
		) {
			cache.current.source = source;
			return cache.current.selection as T;
		}
		cache.current = { source, selection, hasSelection: true, selector };
		return selection;
	}, [equal, selector, store]);
	return useSyncExternalStore(
		store ? store.subscribe : NO_SUBSCRIPTION,
		getSelection,
		getSelection,
	);
}

const NO_SUBSCRIPTION = () => () => undefined;
