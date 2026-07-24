import { ApiRequestError } from "../../api/ApiRequestError";
import type { VersionedObject } from "../../api/types";
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
				let outcome;
				try {
					outcome = await client.updateUserLayout(
						bootstrap.active_show.id,
						session.user.id,
						layout,
						revision,
					);
				} catch (reason) {
					if (!(reason instanceof ApiRequestError) || reason.status !== 409)
						throw reason;
					const current = await client.objectOrNull<StoredDeskLayout>(
						bootstrap.active_show.id,
						"user_layout",
						session.user.id,
					);
					outcome = await client.updateUserLayout(
						bootstrap.active_show.id,
						session.user.id,
						layout,
						current?.revision ?? 0,
					);
				}
				setDeskLayout(
					outcome.object as VersionedObject<StoredDeskLayout>,
				);
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
	};
}
