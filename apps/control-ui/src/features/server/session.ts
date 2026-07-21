import type { ServerController } from "./model";
import type { ServerContextValue } from "./ServerContextValue";

export function createSessionActions(
	model: ServerController,
): Pick<
	ServerContextValue,
	| "createUser"
	| "changeUser"
	| "updateControlDesk"
	| "selectControlDesk"
	| "removeClient"
> {
	const {
		client,
		setError,
		setBootstrap,
		setSession,
	} = model;
	return {
		createUser: async (name) => {
			try {
				if (model.sessionRole !== "primary")
					throw new Error("Only the primary screen can change the desk user");
				setError(null);
				const user = await client.createUser(name);
				setBootstrap(await client.bootstrap());
				await client.closeSession();
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
			await client.closeSession();
			window.location.reload();
		},
		updateControlDesk: async (desk) => {
			try {
				const updated = await client.updateControlDesk(desk);
				setSession((current) =>
					current ? { ...current, desk: updated } : current,
				);
				setBootstrap(await client.bootstrap());
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
				await client.removeClient(deskId);
				setBootstrap(await client.bootstrap());
				setError(null);
				return true;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return false;
			}
		},
	};
}
