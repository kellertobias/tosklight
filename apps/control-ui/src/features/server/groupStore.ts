import type { StoredGroup } from "../../api/types";
import type { ServerController } from "./model";
import type { ServerContextValue } from "./ServerContextValue";
import {
	reconcileShowObject,
	runOptimisticShowObjectMutation,
} from "./showObjectMutations";
import { currentPortableGroups } from "./showObjectSelectors";

export function createGroupStoreActions(
	model: ServerController,
): Pick<ServerContextValue, "storeGroup" | "refreshGroup"> {
	const {
		client,
		setError,
		bootstrap,
		session,
		selectedFixtures,
	} = model;
	return {
		storeGroup: async (id, name, mode = "overwrite") => {
			try {
				if (!bootstrap?.active_show || !session)
					throw new Error("Open a show before storing groups");
				const showId = bootstrap.active_show.id;
				const existing = currentPortableGroups(model).find(
					(item) => item.id === id,
				);
				const programmers = await client.programmers();
				const programmer = programmers.find(
					(item) => item.session_id === session.session_id,
				);
				const expression = programmer?.selection_expression;
				const derived_from =
					expression?.type === "live_group" && expression.group_id
						? {
								source_group_id: expression.group_id,
								rule: expression.rule ?? { type: "all" },
							}
						: (existing?.body.derived_from ?? null);
				const frozen_from =
					expression?.type === "frozen_group" && expression.group_id
						? {
								source_group_id: expression.group_id,
								source_revision: expression.source_revision ?? 0,
								captured_at: new Date().toISOString(),
							}
						: (existing?.body.frozen_from ?? null);
				const numericId = Number(id);
				const scoped = Object.fromEntries(
					Object.entries(programmer?.group_values?.[id] ?? {}).map(
						([attribute, value]) => [attribute, value.value],
					),
				);
				const programming = {
					...(existing?.body.programming ?? {}),
					...scoped,
				};
				const body: StoredGroup = {
					...existing?.body,
					name,
					fixtures:
						mode === "merge"
							? [
									...new Set([
										...(existing?.body.fixtures ?? []),
										...selectedFixtures,
									]),
								]
							: selectedFixtures,
					master: existing?.body.master ?? 1,
					playback_fader:
						existing?.body.playback_fader ??
						(numericId >= 1 && numericId <= 8 ? numericId : null),
					programming,
					derived_from,
					frozen_from,
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
							existing?.revision ?? 0,
						),
				);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		refreshGroup: async (id) => {
			if (!bootstrap?.active_show) {
				setError("Open a show before refreshing a group");
				return false;
			}
			return reconcileShowObject(
				model,
				bootstrap.active_show.id,
				"group",
				id,
			);
		},
	};
}
