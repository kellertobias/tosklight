import type { ServerController } from "./model";
import type { ServerCapabilities } from "./capabilityContracts";

export function createSessionActions(
	model: ServerController,
): Pick<
	ServerCapabilities,
	| "updateControlDesk"
	| "selectControlDesk"
	| "removeClient"
> {
	const { api, setError, setBootstrap, setSession } = model;
	return {
		updateControlDesk: async (desk, options) => {
			try {
				const updated = await api.playback.updateControlDesk(
					desk,
					model.session?.desk,
					options?.hardwareLighting,
				);
				setSession((current) =>
					current ? { ...current, desk: updated } : current,
				);
				setBootstrap(await api.runtime.bootstrap());
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				if (options?.throwOnError) throw reason;
			}
		},
		selectControlDesk: (id) => {
			localStorage.setItem("light.control-desk", id);
			window.location.reload();
		},
		removeClient: async (deskId, clientId) => {
			try {
				await api.playback.removeClient(deskId, clientId);
				setBootstrap(await api.runtime.bootstrap());
				setError(null);
				return true;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return false;
			}
		},
	};
}
