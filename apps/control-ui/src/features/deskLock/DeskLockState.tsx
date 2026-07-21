import {
	createContext,
	type PropsWithChildren,
	useCallback,
	useContext,
	useRef,
	useSyncExternalStore,
} from "react";
import type { DeskLockState } from "../../api/types";
import {
	type DeskLockSnapshot,
	DeskLockStore,
	EMPTY_DESK_LOCK_SNAPSHOT,
} from "./store";

const DeskLockStoreContext = createContext<DeskLockStore | null>(null);

export function DeskLockStateProvider({
	children,
	store,
}: PropsWithChildren<{ store: DeskLockStore }>) {
	return (
		<DeskLockStoreContext.Provider value={store}>
			{children}
		</DeskLockStoreContext.Provider>
	);
}

/** The complete authoritative desk lock, or null before it has been read. */
export function useDeskLock(): DeskLockState | null {
	return useDeskLockSelector(selectDeskLock);
}

/** Whether the desk is currently locked, for readers that only gate on that. */
export function useDeskLocked(): boolean {
	return useDeskLockSelector(selectLocked);
}

function selectDeskLock(snapshot: DeskLockSnapshot) {
	return snapshot.deskLock;
}

function selectLocked(snapshot: DeskLockSnapshot) {
	return snapshot.deskLock?.locked ?? false;
}

/**
 * Equality-cached desk-lock projection.
 *
 * A reader outside a mounted desk-lock boundary observes the inert empty snapshot rather than
 * falling back to broad server state.
 */
function useDeskLockSelector<T>(selector: (snapshot: DeskLockSnapshot) => T): T {
	const store = useContext(DeskLockStoreContext);
	const cache = useRef<{
		source: DeskLockSnapshot | null;
		selection: T | null;
		hasSelection: boolean;
		selector: ((snapshot: DeskLockSnapshot) => T) | null;
	}>({ source: null, selection: null, hasSelection: false, selector: null });
	const getSelection = useCallback(() => {
		const source = store ? store.getSnapshot() : EMPTY_DESK_LOCK_SNAPSHOT;
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
			Object.is(cache.current.selection as T, selection)
		) {
			cache.current.source = source;
			return cache.current.selection as T;
		}
		cache.current = { source, selection, hasSelection: true, selector };
		return selection;
	}, [selector, store]);
	return useSyncExternalStore(
		store ? store.subscribe : NO_SUBSCRIPTION,
		getSelection,
		getSelection,
	);
}

const NO_SUBSCRIPTION = () => () => undefined;
