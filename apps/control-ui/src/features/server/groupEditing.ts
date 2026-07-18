import type { StoredGroup } from "../../api/types";
import type { ServerController } from "./model";
import type { ServerContextValue } from "./ServerContextValue";

export function createGroupEditingActions(
	model: ServerController,
): Pick<
	ServerContextValue,
	"updateGroup" | "setGroupMaster" | "setGroupMasterFlash" | "undoGroup"
> {
	const { client, setError, bootstrap, groups, setGroups } = model;
	return {
		updateGroup: async (id, update) => {
			try {
				if (!bootstrap?.active_show)
					throw new Error("Open a show before editing a group");
				const existing = groups.find((item) => item.id === id);
				if (!existing) throw new Error(`Group ${id} does not exist`);
				const name = update.name?.trim();
				if (!name) throw new Error("Group name is required");
				const body: StoredGroup = {
					...existing.body,
					name,
					color: update.color || undefined,
					icon: update.icon || undefined,
				};
				await client.putObject(
					bootstrap.active_show.id,
					"group",
					id,
					body,
					existing.revision,
				);
				setGroups(
					await client.objects<StoredGroup>(bootstrap.active_show.id, "group"),
				);
				setError(null);
				return true;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return false;
			}
		},
		setGroupMaster: async (id, master) => {
			try {
				await client.setGroupMaster(id, master);
				setGroups((current) =>
					current.map((group) =>
						group.id === id
							? { ...group, body: { ...group.body, master } }
							: group,
					),
				);
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		setGroupMasterFlash: async (id, value) => {
			try {
				await client.setGroupMasterFlash(id, value);
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		undoGroup: async (id) => {
			try {
				if (!bootstrap?.active_show)
					throw new Error("Open a show before undoing a group change");
				const existing = groups.find((item) => item.id === id);
				if (!existing) throw new Error("Group does not exist");
				await client.undoObject(
					bootstrap.active_show.id,
					"group",
					id,
					existing.revision,
				);
				setGroups(
					await client.objects<StoredGroup>(bootstrap.active_show.id, "group"),
				);
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
	};
}
