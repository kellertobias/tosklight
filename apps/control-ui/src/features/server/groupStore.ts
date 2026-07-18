import type { StoredGroup } from "../../api/types";
import type { ServerController } from "./model";
import type { ServerContextValue } from "./ServerContextValue";

export function createGroupStoreActions(
	model: ServerController,
): Pick<ServerContextValue, "storeGroup"> {
	const {
		client,
		setError,
		bootstrap,
		session,
		groups,
		setGroups,
		selectedFixtures,
	} = model;
	return {
		storeGroup: async (id, name, mode = "overwrite") => {
			try {
				if (!bootstrap?.active_show || !session)
					throw new Error("Open a show before storing groups");
				const existing = groups.find((item) => item.id === id);
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
				await client.putObject(
					bootstrap.active_show.id,
					"group",
					id,
					body,
					existing?.revision ?? 0,
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
