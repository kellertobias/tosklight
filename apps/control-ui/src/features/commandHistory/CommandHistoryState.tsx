import {
	createContext,
	type PropsWithChildren,
	useContext,
	useSyncExternalStore,
} from "react";
import type { CommandHistoryEntry } from "../../api/types";

const EMPTY_HISTORY: readonly CommandHistoryEntry[] = [];

/**
 * Authoritative desk command-line history for scoped readers, kept outside the broad
 * server-context update path.
 */
export class CommandHistoryStore {
	private readonly listeners = new Set<() => void>();
	private entries: readonly CommandHistoryEntry[] = EMPTY_HISTORY;

	subscribe = (listener: () => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};

	getSnapshot = () => this.entries;

	install(entries: readonly CommandHistoryEntry[]): void {
		if (this.entries === entries) return;
		this.entries = entries.length === 0 ? EMPTY_HISTORY : entries;
		for (const listener of this.listeners) listener();
	}
}

const CommandHistoryStoreContext = createContext<CommandHistoryStore | null>(
	null,
);

export function CommandHistoryStateProvider({
	children,
	store,
}: PropsWithChildren<{ store: CommandHistoryStore }>) {
	return (
		<CommandHistoryStoreContext.Provider value={store}>
			{children}
		</CommandHistoryStoreContext.Provider>
	);
}

/** The desk command-line history, newest first; empty outside a mounted desk boundary. */
export function useCommandHistory(): readonly CommandHistoryEntry[] {
	const store = useContext(CommandHistoryStoreContext);
	return useSyncExternalStore(
		store ? store.subscribe : NO_SUBSCRIPTION,
		store ? store.getSnapshot : EMPTY_SNAPSHOT,
		store ? store.getSnapshot : EMPTY_SNAPSHOT,
	);
}

const NO_SUBSCRIPTION = () => () => undefined;
const EMPTY_SNAPSHOT = () => EMPTY_HISTORY;
