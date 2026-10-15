import type { TimecodeClockSnapshot } from "../../api/timecodeClockWire";
import { decodeTimecodeClock } from "../../api/timecodeClockWire";
import type { BootstrapSnapshot, SessionResponse } from "../../api/types";

export interface DeskSnapshot {
	/** Null until the desk bootstrap has been loaded for this connection. */
	bootstrap: BootstrapSnapshot | null;
	/** Null until a desk session has been established. */
	session: SessionResponse | null;
	clock?: TimecodeClockSnapshot | null;
	clockReady?: boolean;
	clockError?: string | null;
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
	private clockGeneration = 0;
	private clockRevision = -1;
	clockScope() {
		return this.clockGeneration;
	}
	resetClock() {
		this.clockGeneration++;
		this.clockRevision = -1;
		this.value = {
			...this.value,
			clock: null,
			clockReady: true,
			clockError: null,
		};
		for (const listener of this.listeners) listener();
		return this.clockGeneration;
	}
	installClock(value: unknown, scope = this.clockGeneration) {
		if (scope !== this.clockGeneration) return false;
		const next = decodeTimecodeClock(value);
		if (next.revision <= this.clockRevision) return false;
		this.clockRevision = next.revision;
		this.value = {
			...this.value,
			clock: next,
			clockReady: true,
			clockError: null,
		};
		for (const listener of this.listeners) listener();
		return true;
	}

	invalidateClock(scope = this.clockGeneration, onlyIfEmpty = false) {
		if (scope !== this.clockGeneration || (onlyIfEmpty && this.value.clock))
			return;
		// Keep the accepted revision floor: malformed data must not make older authority admissible.
		this.value = {
			...this.value,
			clock: null,
			clockReady: true,
			clockError: "Timecode status unavailable",
		};
		for (const listener of this.listeners) listener();
	}

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
		const sameSession = this.value.session?.session_id === session?.session_id;
		if (!sameSession) {
			this.clockGeneration++;
			this.clockRevision = -1;
		}
		this.value =
			bootstrap === null && session === null
				? EMPTY
				: {
						bootstrap,
						session,
						clock: sameSession ? this.value.clock : null,
						clockReady: sameSession ? this.value.clockReady : false,
						clockError: sameSession ? this.value.clockError : null,
					};
		for (const listener of this.listeners) listener();
	}
}

export const EMPTY_DESK_SNAPSHOT = EMPTY;
