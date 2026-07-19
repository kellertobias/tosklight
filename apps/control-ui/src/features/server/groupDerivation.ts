import type { StoredGroup } from "../../api/types";
import type { ServerController } from "./model";
import type { ServerContextValue } from "./ServerContextValue";
import { runOptimisticShowObjectMutation } from "./showObjectMutations";

export function createGroupDerivationActions(
	model: ServerController,
): Pick<ServerContextValue, "refreshFrozenGroup" | "detachDerivedGroup"> {
	const {
		client,
		setError,
		bootstrap,
		groups,
		portableGroups,
		setSelectedFixtures,
	} = model;
	return {
		refreshFrozenGroup: async (id) => {
			try {
				if (!bootstrap?.active_show)
					throw new Error("Open a show before refreshing a frozen group");
				const showId = bootstrap.active_show.id;
				const existing = portableGroups.find((item) => item.id === id);
				const sourceId = existing?.body.frozen_from?.source_group_id;
				if (!existing || !sourceId)
					throw new Error("Group is not a frozen group");
				const result = (await client.selectGroup(sourceId, true)) as {
					programmer?: { selected?: string[] };
				};
				const fixtures = result.programmer?.selected ?? [];
				const body: StoredGroup = {
					...existing.body,
					fixtures,
					frozen_from: {
						source_group_id: sourceId,
						source_revision: bootstrap.active_show.revision,
						captured_at: new Date().toISOString(),
					},
				};
				const stored = await runOptimisticShowObjectMutation(
					model,
					showId,
					"group",
					id,
					body,
					() =>
						client.putObject(
							showId,
							"group",
							id,
							body,
							existing.revision,
						),
				);
				if (!stored) return;
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
				const showId = bootstrap.active_show.id;
				const existing = portableGroups.find((item) => item.id === id);
				const projected = groups.find((item) => item.id === id);
				if (!existing?.body.derived_from || !projected)
					throw new Error("Group is not derived");
				const body: StoredGroup = {
					...existing.body,
					fixtures: projected.body.fixtures,
					derived_from: null,
				};
				await runOptimisticShowObjectMutation(
					model,
					showId,
					"group",
					id,
					body,
					() =>
						client.putObject(
							showId,
							"group",
							id,
							body,
							existing.revision,
						),
				);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
	};
}
