import type { ServerController } from "./model";
import type { ServerContextValue } from "./ServerContextValue";

export function createFixtureLibraryActions(
	model: ServerController,
): Pick<
	ServerContextValue,
	| "saveFixtureDefinition"
	| "deleteFixtureDefinition"
	| "saveFixtureProfile"
	| "deleteFixtureProfile"
	| "fixtureProfileRevisions"
	| "saveFixtureProfileSourceGdtf"
	| "importFixturePackage"
	| "exportFixturePackage"
> {
	const {
		client,
		setError,
		setFixtureLibrary,
		setFixtureProfiles,
		setFixtureProfileWarnings,
	} = model;
	return {
		saveFixtureDefinition: async (definition) => {
			try {
				await client.putFixtureDefinition(definition);
				setFixtureLibrary(await client.fixtureLibrary());
				setError(null);
				return true;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return false;
			}
		},
		deleteFixtureDefinition: async (id, revision) => {
			try {
				await client.deleteFixtureDefinition(id, revision);
				setFixtureLibrary(await client.fixtureLibrary());
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		saveFixtureProfile: async (profile, expectedRevision) => {
			try {
				const saved = await client.putFixtureProfile(profile, expectedRevision);
				setFixtureProfiles(await client.fixtureProfiles());
				setFixtureProfileWarnings(await client.fixtureProfileWarnings());
				setFixtureLibrary(await client.fixtureLibrary());
				setError(null);
				return saved;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				throw reason;
			}
		},
		deleteFixtureProfile: async (id, revision) => {
			try {
				await client.deleteFixtureProfile(id, revision);
				setFixtureProfiles(await client.fixtureProfiles());
				setFixtureProfileWarnings(await client.fixtureProfileWarnings());
				setFixtureLibrary(await client.fixtureLibrary());
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		fixtureProfileRevisions: (id) => client.fixtureProfileRevisions(id),
		saveFixtureProfileSourceGdtf: async (id, revision, source) => {
			try {
				await client.putFixtureProfileSourceGdtf(id, revision, source);
				setError(null);
				return true;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return false;
			}
		},
		importFixturePackage: async (source) => {
			try {
				const imported = await client.importFixturePackage(source);
				setFixtureProfiles(await client.fixtureProfiles());
				setFixtureProfileWarnings(await client.fixtureProfileWarnings());
				setFixtureLibrary(await client.fixtureLibrary());
				setError(null);
				return imported;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				throw reason;
			}
		},
		exportFixturePackage: (id, revision) =>
			client.exportFixturePackage(id, revision),
	};
}
