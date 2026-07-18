import type { StoredGroup } from "../../api/types";
import type { ServerController } from "./model";
import type { ServerContextValue } from "./ServerContextValue";

export function createGroupDerivationActions(
	model: ServerController,
): Pick<ServerContextValue, "refreshFrozenGroup" | "detachDerivedGroup"> {
	const {
		client,
		setError,
		bootstrap,
		groups,
		setGroups,
		setSelectedFixtures,
	} = model;
	return {
		refreshFrozenGroup: async (id) => {
			try {
				if (!bootstrap?.active_show)
					throw new Error("Open a show before refreshing a frozen group");
				const existing = groups.find((item) => item.id === id);
				const sourceId = existing?.body.frozen_from?.source_group_id;
				if (!existing || !sourceId)
					throw new Error("Group is not a frozen group");
				const result = (await client.selectGroup(sourceId, true)) as {
					programmer?: { selected?: string[] };
				};
				const fixtures = result.programmer?.selected ?? [];
				await client.putObject(
					bootstrap.active_show.id,
					"group",
					id,
					{
						...existing.body,
						fixtures,
						frozen_from: {
							source_group_id: sourceId,
							source_revision: bootstrap.active_show.revision,
							captured_at: new Date().toISOString(),
						},
					},
					existing.revision,
				);
				setGroups(
					await client.objects<StoredGroup>(bootstrap.active_show.id, "group"),
				);
				setSelectedFixtures(fixtures);
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		detachDerivedGroup: async (id) => {
			try {
				if (!bootstrap?.active_show)
					throw new Error("Open a show before detaching a derived group");
				const existing = groups.find((item) => item.id === id);
				if (!existing?.body.derived_from)
					throw new Error("Group is not derived");
				await client.putObject(
					bootstrap.active_show.id,
					"group",
					id,
					{ ...existing.body, derived_from: null },
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
