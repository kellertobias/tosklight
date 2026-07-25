import type { ServerController } from "./model";
import type { ServerCapabilities } from "./capabilityContracts";

export function createFixtureLibraryActions(
	model: ServerController,
): Pick<
	ServerCapabilities,
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
		api,
		setError,
		setFixtureLibrary,
		setFixtureProfiles,
		setFixtureProfileWarnings,
	} = model;
	return {
		saveFixtureDefinition: async (definition) => {
			try {
				await api.fixtures.putFixtureDefinition(definition);
				setFixtureLibrary(await api.fixtures.fixtureLibrary());
				setError(null);
				return true;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return false;
			}
		},
		deleteFixtureDefinition: async (id, revision) => {
			try {
				await api.fixtures.deleteFixtureDefinition(id, revision);
				setFixtureLibrary(await api.fixtures.fixtureLibrary());
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		saveFixtureProfile: async (profile, expectedRevision) => {
			try {
				const saved = await api.fixtures.putFixtureProfile(profile, expectedRevision);
				setFixtureProfiles(await api.fixtures.fixtureProfiles());
				setFixtureProfileWarnings(await api.fixtures.fixtureProfileWarnings());
				setFixtureLibrary(await api.fixtures.fixtureLibrary());
				setError(null);
				return saved;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				throw reason;
			}
		},
		deleteFixtureProfile: async (id, revision) => {
			try {
				await api.fixtures.deleteFixtureProfile(id, revision);
				setFixtureProfiles(await api.fixtures.fixtureProfiles());
				setFixtureProfileWarnings(await api.fixtures.fixtureProfileWarnings());
				setFixtureLibrary(await api.fixtures.fixtureLibrary());
				setError(null);
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
			}
		},
		fixtureProfileRevisions: (id) => api.fixtures.fixtureProfileRevisions(id),
		saveFixtureProfileSourceGdtf: async (id, revision, source) => {
			try {
				await api.fixtures.putFixtureProfileSourceGdtf(id, revision, source);
				setError(null);
				return true;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				return false;
			}
		},
		importFixturePackage: async (source) => {
			try {
				const imported = await api.fixtures.importFixturePackage(source);
				setFixtureProfiles(await api.fixtures.fixtureProfiles());
				setFixtureProfileWarnings(await api.fixtures.fixtureProfileWarnings());
				setFixtureLibrary(await api.fixtures.fixtureLibrary());
				setError(null);
				return imported;
			} catch (reason) {
				setError(reason instanceof Error ? reason.message : String(reason));
				throw reason;
			}
		},
		exportFixturePackage: (id, revision) =>
			api.fixtures.exportFixturePackage(id, revision),
	};
}
