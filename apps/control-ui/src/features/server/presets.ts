import type { StoredPreset } from "../../api/types";
import {
	normalizePresetFamily,
	presetFamilyAcceptsAttribute,
	presetStorageKey,
} from "../../presetFamilies";
import type { ServerController } from "./model";
import type { ServerContextValue } from "./ServerContextValue";
import { runOptimisticShowObjectMutation } from "./showObjectMutations";
import { currentPresets } from "./showObjectSelectors";

function mergePresetValues(
	existing: Record<string, Record<string, unknown>>,
	incoming: Record<string, Record<string, unknown>>,
	mode: "merge" | "overwrite" | "add_missing_fixtures",
) {
	if (mode === "overwrite") return incoming;
	const values = structuredClone(existing);
	for (const [owner, attributes] of Object.entries(incoming)) {
		if (mode === "add_missing_fixtures" && values[owner]) continue;
		values[owner] = { ...(values[owner] ?? {}), ...attributes };
	}
	return values;
}

export function createPresetActions(
	model: ServerController,
): Pick<ServerContextValue, "applyPreset" | "storePreset"> {
	const {
		client,
		setError,
		bootstrap,
		session,
		setSelectedFixtures,
	} = model;
	return {
		applyPreset: async (address) => {
			try {
				const result = (await client.applyPreset(address)) as
					| { programmer?: { selected?: string[] } }
					| undefined;
				if (result?.programmer?.selected)
					setSelectedFixtures(result.programmer.selected);
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		storePreset: async (address, name, mode, family = address.family) => {
			try {
				if (!bootstrap?.active_show || !session)
					throw new Error("Open a show before storing presets");
				const showId = bootstrap.active_show.id;
				const programmers = await client.programmers();
				const programmer = programmers.find(
					(item) => item.session_id === session.session_id,
				);
				if (!programmer)
					throw new Error("The current programmer is unavailable");
				const values: Record<string, Record<string, unknown>> = {};
				const group_values: Record<
					string,
					Record<string, unknown>
				> = Object.fromEntries(
					Object.entries(programmer.group_values ?? {}).map(
						([group, attributes]) => [
							group,
							Object.fromEntries(
								Object.entries(attributes).map(([attribute, value]) => [
									attribute,
									value.value,
								]),
							),
						],
					),
				);
				const includesAttribute = (attribute: string) => {
					return presetFamilyAcceptsAttribute(
						normalizePresetFamily(family),
						attribute,
					);
				};
				for (const raw of programmer.values) {
					const value = raw as {
						fixture_id: string;
						attribute: string;
						value: unknown;
					};
					if (includesAttribute(value.attribute)) {
						const fixtureValues = values[value.fixture_id] ?? {};
						fixtureValues[value.attribute] = value.value;
						values[value.fixture_id] = fixtureValues;
					}
				}
				for (const attributes of Object.values(group_values))
					for (const attribute of Object.keys(attributes))
						if (!includesAttribute(attribute)) delete attributes[attribute];
				const existing = currentPresets(model).find(
					(item) =>
						normalizePresetFamily(item.body.family) === address.family &&
						item.body.number === address.number,
				);
				const body: StoredPreset = {
					...existing?.body,
					name: name || existing?.body.name || "",
					number: address.number,
					values: mergePresetValues(existing?.body.values ?? {}, values, mode),
					group_values: mergePresetValues(
						existing?.body.group_values ?? {},
						group_values,
						mode,
					),
					family,
				};
				await runOptimisticShowObjectMutation(
					model,
					showId,
					"preset",
					existing?.id ?? presetStorageKey(address),
					body,
					() =>
						client.storePreset(
							showId,
							address,
							{ name, number: address.number, values, group_values, family },
							mode,
							existing?.revision ?? 0,
						),
				);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
	};
}
