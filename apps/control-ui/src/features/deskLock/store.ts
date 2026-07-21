import type { DeskLockState } from "../../api/types";

export interface DeskLockSnapshot {
	/** Null until the desk lock has been read for this session. */
	deskLock: DeskLockState | null;
}

const EMPTY: DeskLockSnapshot = { deskLock: null };

/**
 * Authoritative desk lock for scoped readers.
 *
 * The desk lock is polled twice per second, so an unchanged poll must publish nothing: `install`
 * compares the complete lock state and keeps the previous snapshot when it is equivalent. Without
 * that, every poll would allocate a new object and wake every subscriber.
 */
export class DeskLockStore {
	private readonly listeners = new Set<() => void>();
	private value: DeskLockSnapshot = EMPTY;

	subscribe = (listener: () => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};

	getSnapshot = () => this.value;

	install(deskLock: DeskLockState | null): void {
		if (equalDeskLock(this.value.deskLock, deskLock)) return;
		this.value = deskLock === null ? EMPTY : { deskLock };
		for (const listener of this.listeners) listener();
	}
}

export function equalDeskLock(
	left: DeskLockState | null,
	right: DeskLockState | null,
) {
	if (left === right) return true;
	if (!left || !right) return false;
	return (
		left.locked === right.locked &&
		left.message === right.message &&
		left.wallpaper === right.wallpaper &&
		left.unlock_mode === right.unlock_mode
	);
}

export const EMPTY_DESK_LOCK_SNAPSHOT = EMPTY;
