import {
	createContext,
	type PropsWithChildren,
	useContext,
	useMemo,
} from "react";
import type { ConfigurationUpdateResult } from "../../api/client/configuration";
import type { DeskConfiguration } from "../../api/types";
import type { ConfigurationStore } from "./store";

export type ControlTimingInput = Partial<
	Pick<
		DeskConfiguration,
		| "speed_groups_bpm"
		| "programmer_fade_millis"
		| "sequence_master_fade_millis"
	>
>;

export interface ConfigurationActions {
	/** Returns whether the desk reported that the change requires a restart. */
	saveConfiguration(next: DeskConfiguration): Promise<boolean>;
	/** Merges timing settings onto the authoritative configuration. */
	setControlTiming(input: ControlTimingInput): Promise<void>;
}

interface ConfigurationActionsProviderProps {
	store: ConfigurationStore;
	updateConfiguration(
		next: DeskConfiguration,
	): Promise<ConfigurationUpdateResult>;
	onApplied(result: ConfigurationUpdateResult): void;
	onError(message: string | null): void;
}

const ConfigurationActionsContext = createContext<ConfigurationActions | null>(
	null,
);

/** Mounting this action boundary performs no reads and no network work. */
export function ConfigurationActionsProvider({
	children,
	store,
	updateConfiguration,
	onApplied,
	onError,
}: PropsWithChildren<ConfigurationActionsProviderProps>) {
	const actions = useMemo<ConfigurationActions>(() => {
		const apply = async (next: DeskConfiguration) => {
			const result = await updateConfiguration(next);
			onApplied(result);
			onError(null);
			return result;
		};
		return {
			saveConfiguration: async (next) => {
				try {
					return (await apply(next)).requires_restart;
				} catch (reason) {
					onError(asMessage(reason));
					return false;
				}
			},
			setControlTiming: async (input) => {
				// Merge onto the authoritative configuration so a timing write never reverts a
				// setting this desk has not observed yet.
				const configuration = store.getSnapshot().configuration;
				if (!configuration) return;
				try {
					await apply({ ...configuration, ...input });
				} catch (reason) {
					onError(asMessage(reason));
				}
			},
		};
	}, [onApplied, onError, store, updateConfiguration]);
	return (
		<ConfigurationActionsContext.Provider value={actions}>
			{children}
		</ConfigurationActionsContext.Provider>
	);
}

export function useConfigurationActions(): ConfigurationActions | null {
	return useContext(ConfigurationActionsContext);
}

function asMessage(reason: unknown) {
	return reason instanceof Error ? reason.message : String(reason);
}
