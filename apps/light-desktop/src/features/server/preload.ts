import { ApiRequestError } from "../../api/ApiRequestError";
import type { ServerController } from "./model";
import type { ServerCapabilities } from "./capabilityContracts";
import { reconcileShowObject } from "./showObjectMutations";

export function createPreloadActions(
	model: ServerController,
): Pick<ServerCapabilities, "storePreload"> {
	const {
		api,
		setError,
		bootstrap,
		refresh,
	} = model;
	return {
		storePreload: async (input, revision) => {
			try {
				if (!bootstrap?.active_show)
					throw new Error("Open a show before storing preload data");
				let response;
				try {
					response = await api.showObjects.storePreload(
						bootstrap.active_show.id,
						input,
						revision,
					);
				} catch (reason) {
					if (!(reason instanceof ApiRequestError) || reason.status !== 409)
						throw reason;
					const kind = input.target === "preset" ? "preset" : "cue_list";
					const current = await api.showObjects.objectOrNull(
						bootstrap.active_show.id,
						kind,
						input.target_id,
					);
					response = await api.showObjects.storePreload(
						bootstrap.active_show.id,
						input,
						current?.revision ?? 0,
					);
				}
				if (input.target === "preset") {
					const reconciled = await reconcileShowObject(
						model,
						bootstrap.active_show.id,
						"preset",
						input.target_id,
						response.event_sequence ?? null,
					);
					if (!reconciled) return false;
				} else await refresh();
				setError(null);
				return true;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return false;
			}
		},
	};
}
