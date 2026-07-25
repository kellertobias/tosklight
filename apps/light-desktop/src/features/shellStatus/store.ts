import type { ConnectionStatus } from "../../api/types";

export interface ShellStatusSnapshot {
	status: ConnectionStatus;
	/** The most recent operator-visible error, or null once it has been cleared. */
	error: string | null;
}

const INITIAL: ShellStatusSnapshot = { status: "connecting", error: null };

/**
 * Connection status and the shared operator error banner.
 *
 * Both are scalars written from many places across the app. Keeping them out of the broad server
 * context means an error raised by one feature no longer rerenders every unrelated consumer.
 */
export class ShellStatusStore {
	private readonly listeners = new Set<() => void>();
	private value: ShellStatusSnapshot = INITIAL;

	subscribe = (listener: () => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};

	getSnapshot = () => this.value;

	install(status: ConnectionStatus, error: string | null): void {
		if (this.value.status === status && this.value.error === error) return;
		this.value = { status, error };
		for (const listener of this.listeners) listener();
	}
}

export const INITIAL_SHELL_STATUS_SNAPSHOT = INITIAL;
