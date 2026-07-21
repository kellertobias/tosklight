import type { StoredGroup } from "../../api/types";
import type { ServerController } from "./model";
import type { ServerContextValue } from "./ServerContextValue";
import {
	reconcileShowObject,
	runOptimisticShowObjectMutation,
} from "./showObjectMutations";
import { currentPortableGroups } from "./showObjectSelectors";

export function createGroupEditingActions(
	model: ServerController,
): Pick<ServerContextValue, "updateGroup" | "undoGroup"> {
	const { client, setError, bootstrap } = model;
	return {
		updateGroup: async (id, update) => {
			try {
				if (!bootstrap?.active_show)
					throw new Error("Open a show before editing a group");
				const showId = bootstrap.active_show.id;
				const existing = currentPortableGroups(model).find(
					(item) => item.id === id,
				);
				if (!existing) throw new Error(`Group ${id} does not exist`);
				const name = update.name?.trim();
				if (!name) throw new Error("Group name is required");
				const body: StoredGroup = {
					...existing.body,
					name,
					color: update.color || undefined,
					icon: update.icon || undefined,
				};
				return runOptimisticShowObjectMutation(
					model,
					showId,
					"group",
					id,
					body,
					() => client.putObject(showId, "group", id, body, existing.revision),
				);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return false;
			}
		},
		undoGroup: async (id) => {
			try {
				if (!bootstrap?.active_show)
					throw new Error("Open a show before undoing a group change");
				const existing = currentPortableGroups(model).find(
					(item) => item.id === id,
				);
				if (!existing) throw new Error("Group does not exist");
				const response = await client.undoObject(
					bootstrap.active_show.id,
					"group",
					id,
					existing.revision,
				);
				await reconcileShowObject(
					model,
					bootstrap.active_show.id,
					"group",
					id,
					response.event_sequence,
				);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
	};
}
