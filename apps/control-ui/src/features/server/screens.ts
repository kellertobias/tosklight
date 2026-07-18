import type { ServerController } from "./model";
import type { ServerContextValue } from "./ServerContextValue";

export function createScreenActions(
	model: ServerController,
): Pick<ServerContextValue, "saveScreen" | "deleteScreen" | "setScreenPage"> {
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
