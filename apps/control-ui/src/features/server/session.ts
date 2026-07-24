import type { ServerController } from "./model";
import type { ServerCapabilities } from "./capabilityContracts";

export function createSessionActions(
	model: ServerController,
): Pick<
	ServerCapabilities,
	| "createUser"
	| "changeUser"
	| "updateControlDesk"
	| "selectControlDesk"
	| "removeClient"
> {
	const { api, setError, setBootstrap, setSession } = model;
	return {
		createUser: async (name) => {
			try {
				if (model.sessionRole !== "primary")
					throw new Error("Only the primary screen can change the desk user");
				setError(null);
				const user = await api.desk.createUser(name);
				setBootstrap(await api.runtime.bootstrap());
				await api.runtime.closeSession();
				localStorage.setItem("light.operator", user.name);
				window.location.reload();
			} catch (caught) {
				setError(caught instanceof Error ? caught.message : String(caught));
			}
		},
		changeUser: async (user) => {
			if (model.sessionRole !== "primary") {
				setError("Only the primary screen can change the desk user");
				return;
			}
			localStorage.setItem("light.operator", user.name);
			await api.runtime.closeSession();
			window.location.reload();
		},
		updateControlDesk: async (desk) => {
			try {
				const updated = await api.playback.updateControlDesk(
					desk,
					model.session?.desk,
				);
				setSession((current) =>
					current ? { ...current, desk: updated } : current,
				);
				setBootstrap(await api.runtime.bootstrap());
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		selectControlDesk: (id) => {
			localStorage.setItem("light.control-desk", id);
			window.location.reload();
		},
		removeClient: async (deskId) => {
			try {
				await api.playback.removeClient(deskId);
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
