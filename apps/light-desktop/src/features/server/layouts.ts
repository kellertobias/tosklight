import { ApiRequestError } from "../../api/ApiRequestError";
import type { VersionedObject } from "../../api/types";
import { DESK_LAYOUT_ID, type StoredDeskLayout } from "./contracts";
import type { ServerController } from "./model";
import type { ServerCapabilities } from "./capabilityContracts";

export function createLayoutActions(
	model: ServerController,
): Pick<ServerCapabilities, "saveDeskLayout"> {
	const {
		api,
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
				// A layout this show already holds keeps its own id, so a show written before the
				// collapse is updated in place rather than left beside a second one.
				const layoutId = deskLayout?.id ?? DESK_LAYOUT_ID;
				const revision = deskLayout?.revision ?? 0;
				let outcome;
				try {
					outcome = await api.showObjects.updateUserLayout(
						bootstrap.active_show.id,
						layoutId,
						layout,
						revision,
					);
				} catch (reason) {
					if (!(reason instanceof ApiRequestError) || reason.status !== 409)
						throw reason;
					const current = await api.showObjects.objectOrNull<StoredDeskLayout>(
						bootstrap.active_show.id,
						"user_layout",
						layoutId,
					);
					outcome = await api.showObjects.updateUserLayout(
						bootstrap.active_show.id,
						layoutId,
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
