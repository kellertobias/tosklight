import type { LightApiClient } from "../../api/LightApiClient";
import type { ScreenSnapshot } from "../../api/types";
import type { ScreenCapabilities } from "./types";

interface ScreenActionDependencies {
	client: LightApiClient;
	setError: (error: string | null) => void;
	setScreens: (screens: ScreenSnapshot | null) => void;
}

export function createScreenActions(
	model: ScreenActionDependencies,
): Omit<ScreenCapabilities, "screens"> {
	const { client, setError, setScreens } = model;
	return {
		saveScreen: async (screen) => {
			try {
				await client.putScreen(screen);
				setScreens(await client.screens());
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		deleteScreen: async (id) => {
			try {
				await client.deleteScreen(id);
				setScreens(await client.screens());
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		setScreenPage: async (id, page) => {
			try {
				await client.setScreenPage(id, page);
				setScreens(await client.screens());
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
	};
}
