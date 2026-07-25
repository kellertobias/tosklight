import {
	createContext,
	type PropsWithChildren,
	useContext,
	useSyncExternalStore,
} from "react";
import type { HighlightAction, HighlightState } from "../../api/types";
import {
	EMPTY_HIGHLIGHT_SNAPSHOT,
	type HighlightSnapshot,
	HighlightStore,
} from "./store";

const HighlightStoreContext = createContext<HighlightStore | null>(null);

export function HighlightStateProvider({
	children,
	store,
}: PropsWithChildren<{ store: HighlightStore }>) {
	return (
		<HighlightStoreContext.Provider value={store}>
			{children}
		</HighlightStoreContext.Provider>
	);
}

export interface HighlightActions {
	highlightAction: (action: HighlightAction) => Promise<boolean>;
	dismissHighlightError: () => void;
	/** Patch-preview DMX highlight for the fixtures being placed; false clears it. */
	setPatchPreviewHighlight: (
		active: boolean,
		fixtureIds?: string[],
	) => Promise<boolean>;
}

const HighlightActionsContext = createContext<HighlightActions | null>(null);

export function HighlightActionsProvider({
	children,
	actions,
}: PropsWithChildren<{ actions: HighlightActions }>) {
	return (
		<HighlightActionsContext.Provider value={actions}>
			{children}
		</HighlightActionsContext.Provider>
	);
}

/** Highlight runtime state, or null while unavailable or outside a desk boundary. */
export function useHighlightSnapshot(): HighlightState | null {
	return useHighlightStoreSnapshot().highlight;
}

/** The operator-facing Highlight failure, or null when the last action applied. */
export function useHighlightErrorMessage(): string | null {
	return useHighlightStoreSnapshot().error;
}

/** Highlight actions, or null outside a mounted desk boundary. */
export function useHighlightActions(): HighlightActions | null {
	return useContext(HighlightActionsContext);
}

function useHighlightStoreSnapshot(): HighlightSnapshot {
	const store = useContext(HighlightStoreContext);
	return useSyncExternalStore(
		store ? store.subscribe : NO_SUBSCRIPTION,
		store ? store.getSnapshot : EMPTY_SNAPSHOT,
		store ? store.getSnapshot : EMPTY_SNAPSHOT,
	);
}

const NO_SUBSCRIPTION = () => () => undefined;
const EMPTY_SNAPSHOT = () => EMPTY_HIGHLIGHT_SNAPSHOT;
