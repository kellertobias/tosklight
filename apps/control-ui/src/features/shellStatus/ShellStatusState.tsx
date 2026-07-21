import {
	createContext,
	type PropsWithChildren,
	useCallback,
	useContext,
	useRef,
	useSyncExternalStore,
} from "react";
import type { ConnectionStatus } from "../../api/types";
import {
	INITIAL_SHELL_STATUS_SNAPSHOT,
	type ShellStatusSnapshot,
	ShellStatusStore,
} from "./store";

const ShellStatusStoreContext = createContext<ShellStatusStore | null>(null);

export function ShellStatusStateProvider({
	children,
	store,
}: PropsWithChildren<{ store: ShellStatusStore }>) {
	return (
		<ShellStatusStoreContext.Provider value={store}>
			{children}
		</ShellStatusStoreContext.Provider>
	);
}

/** The current server connection status. */
export function useConnectionStatus(): ConnectionStatus {
	return useShellStatusSelector(selectStatus);
}

/** The shared operator error banner text, or null when there is nothing to report. */
export function useServerError(): string | null {
	return useShellStatusSelector(selectError);
}

function selectStatus(snapshot: ShellStatusSnapshot) {
	return snapshot.status;
}

function selectError(snapshot: ShellStatusSnapshot) {
	return snapshot.error;
}

/**
 * Equality-cached shell-status projection.
 *
 * A status reader is not woken by an error change and vice versa, and a reader outside a mounted
 * boundary observes the initial snapshot rather than falling back to broad server state.
 */
function useShellStatusSelector<T>(
	selector: (snapshot: ShellStatusSnapshot) => T,
): T {
	const store = useContext(ShellStatusStoreContext);
	const cache = useRef<{
		source: ShellStatusSnapshot | null;
		selection: T | null;
		hasSelection: boolean;
		selector: ((snapshot: ShellStatusSnapshot) => T) | null;
	}>({ source: null, selection: null, hasSelection: false, selector: null });
	const getSelection = useCallback(() => {
		const source = store
			? store.getSnapshot()
			: INITIAL_SHELL_STATUS_SNAPSHOT;
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
