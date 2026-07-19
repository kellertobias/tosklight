import type {
	FixtureDefinition,
	FixtureProfile,
} from "../../../api/types";
import { fixtureDefinitionsFromProfiles } from "./definitions";

export function fixtureDefinitionKey(definition: FixtureDefinition) {
	return `${definition.profile_id ?? definition.id}:${definition.revision}:${definition.mode_id ?? definition.id}`;
}

function migratedLegacyContentKey(definition: FixtureDefinition) {
	const normalized = (value: string) => value.trim().toLocaleLowerCase();
	const physical = definition.physical;
	return JSON.stringify([
		normalized(definition.manufacturer),
		normalized(definition.model),
		normalized(definition.name || definition.model),
		normalized(definition.mode),
		normalized(definition.device_type),
		definition.footprint,
		physical.width_millimetres ?? null,
		physical.height_millimetres ?? null,
		physical.depth_millimetres ?? null,
		physical.weight_kilograms ?? null,
		physical.power_watts ?? null,
	]);
}

/** Prefer profile-backed modes and hide their retained schema-v1 migration sources. */
export function mergeFixtureDefinitions(
	profiles: FixtureProfile[],
	legacyDefinitions: FixtureDefinition[],
) {
	const profileDefinitions = fixtureDefinitionsFromProfiles(profiles);
	const exactKeys = new Set(profileDefinitions.map(fixtureDefinitionKey));
	const migratedKeys = new Set(profileDefinitions.map(migratedLegacyContentKey));
	return [
		...profileDefinitions,
		...legacyDefinitions.filter(
			(definition) =>
				!exactKeys.has(fixtureDefinitionKey(definition)) &&
				!migratedKeys.has(migratedLegacyContentKey(definition)),
		),
	];
}
