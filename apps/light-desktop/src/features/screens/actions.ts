import { formatErrorDetails } from "@tosklight/ui";
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
): Omit<ScreenCapabilities, "screens" | "surfaces"> {
	const { api, setError, setScreens } = model;
	return {
		saveScreen: async (screen, options) => {
			try {
				await api.playback.putScreen(screen);
				setScreens(await api.playback.screens());
				setError(null);
			} catch (reason) {
				setError(formatErrorDetails(reason));
				if (options?.throwOnError) throw reason;
			}
		},
		deleteScreen: async (id) => {
			try {
				await api.playback.deleteScreen(id);
				setScreens(await api.playback.screens());
				setError(null);
			} catch (reason) {
				setError(formatErrorDetails(reason));
			}
		},
		setSurfacePage: async (id, page) => {
			try {
				await api.playback.setSurfacePage(id, page);
				setScreens(await api.playback.screens());
				setError(null);
			} catch (reason) {
				setError(formatErrorDetails(reason));
			}
		},
		selectSurface: async (id) => {
			try {
				await api.playback.selectSurface(id);
				setScreens(await api.playback.screens());
				setError(null);
			} catch (reason) {
				setError(formatErrorDetails(reason));
			}
		},
		updatePlaybackSurface: async (surfaceId, patch) => {
			try {
				await api.playback.updatePlaybackSurface(surfaceId, patch);
				setScreens(await api.playback.screens());
				setError(null);
			} catch (reason) {
				setError(formatErrorDetails(reason));
				throw reason;
			}
		},
		updateProgrammerControlSurface: async (patch) => {
			try {
				await api.playback.updateProgrammerControlSurface(patch);
				setScreens(await api.playback.screens());
				setError(null);
			} catch (reason) {
				setError(formatErrorDetails(reason));
			}
		},
	};
}
