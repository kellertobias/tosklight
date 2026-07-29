export const CURRENT_FIXTURE_PROFILE_SCHEMA_VERSION = 3;

type ProfileSnapshot = {
	schema_version: number;
};

type FixtureDefinitionWithSnapshot = {
	schema_version: number;
	profile_snapshot?: ProfileSnapshot | null;
};

/**
 * The frontend fixture editor currently projects schema-v2 view models. E2E tests that write a
 * new patched_fixture must seed the current portable fixture schema instead of relying on the
 * server's legacy-read migration.
 */
export function currentFixtureDefinition<
	T extends FixtureDefinitionWithSnapshot,
>(definition: T): T {
	if (!definition.profile_snapshot)
		throw new Error("Current fixture definitions require a profile snapshot");
	return {
		...definition,
		schema_version: CURRENT_FIXTURE_PROFILE_SCHEMA_VERSION,
		profile_snapshot: {
			...definition.profile_snapshot,
			schema_version: CURRENT_FIXTURE_PROFILE_SCHEMA_VERSION,
		},
	};
}
