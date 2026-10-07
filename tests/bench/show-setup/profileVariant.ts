import type { ApiDriver } from "../core/api";

/** A shipped profile as the fixture library returns it, loosely typed for test-only edits. */
// biome-ignore lint/suspicious/noExplicitAny: test-only structural edits of library JSON.
export type LibraryProfileJson = Record<string, any>;

/**
 * Saves a user-library copy of a shipped profile under a new identity after `edit` changes it.
 * Shipped profiles derive missing physical data at load, so a scenario that needs a fixture
 * without that data authors the gap on a copy instead of relying on a shipped profile lacking it.
 * A copy already saved under `identity` on this server is reused.
 */
export async function saveProfileVariant(
	api: ApiDriver,
	source: { manufacturer: string; profile: string },
	identity: { manufacturer: string; name: string },
	edit: (profile: LibraryProfileJson) => void,
): Promise<LibraryProfileJson> {
	const library = await api.request<{ profiles: LibraryProfileJson[] }>("GET", "/api/v2/fixture-library/profiles");
	const existing = library.profiles.find(
		(candidate) => candidate.manufacturer === identity.manufacturer && candidate.name === identity.name,
	);
	if (existing) return existing;
	const shipped = library.profiles.find(
		(candidate) => candidate.manufacturer === source.manufacturer && candidate.name === source.profile,
	);
	if (!shipped) throw new Error(`The fixture library has no ${source.manufacturer} ${source.profile}`);
	const profile = structuredClone(shipped);
	profile.id = crypto.randomUUID();
	profile.revision = 0;
	profile.manufacturer = identity.manufacturer;
	profile.name = identity.name;
	edit(profile);
	await api.fixtureLibraryAction({ type: "save_profile", profile, expected_revision: 0 });
	return profile;
}

/** Every continuous function of `attribute` in every mode. */
export function continuousFunctions(profile: LibraryProfileJson, attribute: string): LibraryProfileJson[] {
	return profile.modes.flatMap((mode: LibraryProfileJson) =>
		mode.channels
			.filter((channel: LibraryProfileJson) => channel.attribute === attribute)
			.flatMap((channel: LibraryProfileJson) => channel.functions)
			.filter((fn: LibraryProfileJson) => fn.behavior?.type === "continuous"),
	);
}

/**
 * Shipped profiles may use attributes a user-library save would ask the operator to map first;
 * a copy maps every channel and function attribute under `prefix` to the registered `control`.
 */
export function mapAttributesToControl(profile: LibraryProfileJson, prefix: string) {
	for (const mode of profile.modes)
		for (const channel of mode.channels) {
			if (channel.attribute.startsWith(prefix)) channel.attribute = "control";
			for (const fn of channel.functions) if (fn.attribute.startsWith(prefix)) fn.attribute = "control";
		}
}
