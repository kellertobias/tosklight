import type { StoredDeskLayout } from "./contracts";
import type { ServerController } from "./model";
import type { ServerContextValue } from "./ServerContextValue";

export function createLayoutActions(
	model: ServerController,
): Pick<ServerContextValue, "saveDeskLayout"> {
	const {
		client,
		setError,
		bootstrap,
		session,
		deskLayout,
		setDeskLayout,
	} = model;
	return {
		saveDeskLayout: async (layout) => {
			try {
				if (!bootstrap?.active_show || !session)
					throw new Error("Open a show before saving a Desktop layout");
				const revision = deskLayout?.revision ?? 0;
				await client.putObject(
					bootstrap.active_show.id,
					"user_layout",
					session.user.id,
					layout,
					revision,
				);
				const layouts = await client.objects<StoredDeskLayout>(
					bootstrap.active_show.id,
					"user_layout",
				);
				setDeskLayout(
					layouts.find((item) => item.id === session.user.id) ?? null,
				);
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
	};
}
