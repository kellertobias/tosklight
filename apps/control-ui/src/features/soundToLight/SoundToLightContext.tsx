import { createContext, type PropsWithChildren, useContext } from "react";
import type {
	SoundObservation,
	SoundToLightConfig,
	SpeedGroupActionInput,
	SpeedGroupId,
	SpeedGroupSoundState,
} from "../../api/types";

/**
 * Scoped Sound-to-Light speed-group calls for the sound capture and configuration
 * surfaces, so they stay off the broad server-context path.
 */
export interface SoundToLightActions {
	speedGroup: (group: SpeedGroupId) => Promise<SpeedGroupSoundState>;
	updateSpeedGroup: (
		group: SpeedGroupId,
		configuration: SoundToLightConfig,
	) => Promise<SpeedGroupSoundState>;
	observeSpeedGroup: (
		group: SpeedGroupId,
		observation: SoundObservation,
	) => Promise<SpeedGroupSoundState>;
	speedGroupAction: (
		group: SpeedGroupId,
		input: SpeedGroupActionInput,
	) => Promise<SpeedGroupSoundState>;
}

const SoundToLightContext = createContext<SoundToLightActions | null>(null);

export function SoundToLightProvider({
	children,
	actions,
}: PropsWithChildren<{ actions: SoundToLightActions }>) {
	return (
		<SoundToLightContext.Provider value={actions}>
			{children}
		</SoundToLightContext.Provider>
	);
}

/** Sound-to-Light actions, or null outside a mounted desk boundary. */
export function useSoundToLightActions(): SoundToLightActions | null {
	return useContext(SoundToLightContext);
}
