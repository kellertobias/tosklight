import type { HighlightState } from "../../api/types";

export interface HighlightSnapshot {
	/** Null until the desk has reported Highlight state for this session. */
	highlight: HighlightState | null;
	/** Operator-facing Highlight failure, or null when the last action applied. */
	error: string | null;
}

const EMPTY: HighlightSnapshot = { highlight: null, error: null };

/**
 * Authoritative Highlight runtime state for scoped readers.
 *
 * The broad server state remains the writer; this store lets Highlight surfaces
 * rerender without observing the whole server context.
 */
export class HighlightStore {
	private readonly listeners = new Set<() => void>();
	private value: HighlightSnapshot = EMPTY;

	subscribe = (listener: () => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};

	getSnapshot = () => this.value;

	install(highlight: HighlightState | null, error: string | null): void {
		if (this.value.highlight === highlight && this.value.error === error)
			return;
		this.value =
			highlight === null && error === null ? EMPTY : { highlight, error };
		for (const listener of this.listeners) listener();
	}
}

export const EMPTY_HIGHLIGHT_SNAPSHOT = EMPTY;
