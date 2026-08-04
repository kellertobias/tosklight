import type { LightApi } from "../../api/client/api";
import type { ScreenSnapshot } from "../../api/types";
import type { ScreenCapabilities } from "./types";

interface ScreenActionDependencies {
	api: LightApi;
	setError: (error: string | null) => void;
	setScreens: (screens: ScreenSnapshot | null) => void;
}

export function createScreenActions(
	model: ScreenActionDependencies,
): Omit<ScreenCapabilities, "screens"> {
	const { api, setError, setScreens } = model;
	return {
		saveScreen: async (screen) => {
			try {
				await api.playback.putScreen(screen);
				setScreens(await api.playback.screens());
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		deleteScreen: async (id) => {
			try {
				await api.playback.deleteScreen(id);
				setScreens(await api.playback.screens());
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		setScreenPage: async (id, page) => {
			try {
				await api.playback.setScreenPage(id, page);
				setScreens(await api.playback.screens());
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		updateProgrammerControlSurface: async (patch) => {
			try {
				await api.playback.updateProgrammerControlSurface(patch);
				setScreens(await api.playback.screens());
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
	};
}
