import type { BootstrapSnapshot, SessionResponse } from "../../api/types";

export interface DeskSnapshot {
	/** Null until the desk bootstrap has been loaded for this connection. */
	bootstrap: BootstrapSnapshot | null;
	/** Null until a desk session has been established. */
	session: SessionResponse | null;
}

const EMPTY: DeskSnapshot = { bootstrap: null, session: null };

/**
 * Authoritative desk bootstrap/session snapshot for scoped readers.
 *
 * Readers select one value with an equality-cached selector, so replacing the bootstrap on a
 * poll only rerenders the consumers whose own selection actually changed.
 */
export class DeskSnapshotStore {
	private readonly listeners = new Set<() => void>();
	private value: DeskSnapshot = EMPTY;

	subscribe = (listener: () => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};

	getSnapshot = () => this.value;

	install(
		bootstrap: BootstrapSnapshot | null,
		session: SessionResponse | null,
	): void {
		if (this.value.bootstrap === bootstrap && this.value.session === session)
			return;
		this.value =
			bootstrap === null && session === null ? EMPTY : { bootstrap, session };
		for (const listener of this.listeners) listener();
	}
}

export const EMPTY_DESK_SNAPSHOT = EMPTY;
