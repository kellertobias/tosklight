import { ApiRequestError } from "../../api/ApiRequestError";
import type { ServerController } from "./model";
import type { ServerContextValue } from "./ServerContextValue";
import { reconcileShowObject } from "./showObjectMutations";

export function createPreloadActions(
	model: ServerController,
): Pick<ServerContextValue, "storePreload" | "storeDynamic"> {
	const {
		client,
		setError,
		bootstrap,
		cueObjects,
		selectedFixtures,
		selectedGroupId,
		refresh,
	} = model;
	return {
		storePreload: async (input, revision) => {
			try {
				if (!bootstrap?.active_show)
					throw new Error("Open a show before storing preload data");
				let response;
				try {
					response = await client.storePreload(
						bootstrap.active_show.id,
						input,
						revision,
					);
				} catch (reason) {
					if (!(reason instanceof ApiRequestError) || reason.status !== 409)
						throw reason;
					const kind = input.target === "preset" ? "preset" : "cue_list";
					const current = await client.objectOrNull(
						bootstrap.active_show.id,
						kind,
						input.target_id,
					);
					response = await client.storePreload(
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
		storeDynamic: async (speed, width, direction) => {
			try {
				if (!bootstrap?.active_show)
					throw new Error("Open a show before storing a dynamic");
				const target = cueObjects[0];
				if (!target)
					throw new Error("Create a Cuelist before storing a dynamic");
				try {
					await client.recordDynamic(
						bootstrap.active_show.id,
						target.id,
						target.revision,
						{
							speed,
							width,
							direction,
							fixtureIds: selectedGroupId ? [] : selectedFixtures,
							groupIds: selectedGroupId ? [selectedGroupId] : [],
						},
					);
				} catch (reason) {
					if (!(reason instanceof ApiRequestError) || reason.status !== 409)
						throw reason;
					const current = await client.object(
						bootstrap.active_show.id,
						"cue_list",
						target.id,
					);
					await client.recordDynamic(
						bootstrap.active_show.id,
						target.id,
						current.revision,
						{
							speed,
							width,
							direction,
							fixtureIds: selectedGroupId ? [] : selectedFixtures,
							groupIds: selectedGroupId ? [selectedGroupId] : [],
						},
					);
				}
				await refresh();
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
	};
}
