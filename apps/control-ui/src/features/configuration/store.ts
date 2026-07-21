import type { DeskConfiguration } from "../../api/types";

export interface ConfigurationSnapshot {
	/** Null until the desk configuration has been loaded for this session. */
	configuration: DeskConfiguration | null;
}

const EMPTY: ConfigurationSnapshot = { configuration: null };

/**
 * Authoritative desk configuration for scoped readers.
 *
 * Readers select one scalar setting, so replacing the configuration only rerenders the consumers
 * whose own value actually changed.
 */
export class ConfigurationStore {
	private readonly listeners = new Set<() => void>();
	private value: ConfigurationSnapshot = EMPTY;

	subscribe = (listener: () => void) => {
		this.listeners.add(listener);
		return () => this.listeners.delete(listener);
	};

	getSnapshot = () => this.value;

	install(configuration: DeskConfiguration | null): void {
		if (this.value.configuration === configuration) return;
		this.value = configuration === null ? EMPTY : { configuration };
		for (const listener of this.listeners) listener();
	}
}

export const EMPTY_CONFIGURATION_SNAPSHOT = EMPTY;
