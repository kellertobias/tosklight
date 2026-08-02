import { createContext, type PropsWithChildren, useContext } from "react";
import type {
	FixtureAttributeMapping,
	FixturePackageImportOutcome,
	GelCatalog,
	GelCatalogImportPreview,
	GelCatalogImportTarget,
} from "../../api/client/fixtures";
import type {
	FixtureDefinition,
	FixtureProfile,
	PatchLayer,
	VersionedObject,
} from "../../api/types";

/**
 * Scoped fixture-library desk state and transfer actions for the library and patch setup
 * surfaces: transferable profiles, legacy definitions, warnings, patch layers, and the
 * profile save/delete/revision/package calls.
 */
export interface FixtureLibraryState {
	fixtureLibrary: FixtureDefinition[];
	fixtureProfiles: FixtureProfile[];
	fixtureProfileWarnings: string[];
	patchLayers: VersionedObject<PatchLayer>[];
	unresolvedMvrFixtures: VersionedObject<Record<string, unknown>>[];
	savePatchLayer: (layer: PatchLayer) => Promise<boolean>;
	saveFixtureProfile: (
		profile: FixtureProfile,
		expectedRevision: number,
	) => Promise<FixtureProfile>;
	deleteFixtureProfile: (id: string, revision: number) => Promise<void>;
	fixtureProfileRevisions: (id: string) => Promise<FixtureProfile[]>;
	saveFixtureProfileSourceGdtf: (
		id: string,
		revision: number,
		source: Uint8Array,
	) => Promise<boolean>;
	importFixturePackage: (
		source: Uint8Array,
		attributeMappings?: FixtureAttributeMapping[],
	) => Promise<FixturePackageImportOutcome>;
	exportFixturePackage: (id: string, revision: number) => Promise<Blob>;
	gelCatalogs?: (query?: string) => Promise<GelCatalog[]>;
	previewGelCatalogCsvImport?: (input: {
		target: GelCatalogImportTarget;
		catalogName: string;
		csv: Uint8Array;
	}) => Promise<GelCatalogImportPreview>;
	confirmGelCatalogCsvImport?: (input: {
		target: GelCatalogImportTarget;
		catalogName: string;
		csv: Uint8Array;
	}) => Promise<GelCatalog>;
}

const FixtureLibraryContext = createContext<FixtureLibraryState | null>(null);

export function FixtureLibraryProvider({
	children,
	library,
}: PropsWithChildren<{ library: FixtureLibraryState }>) {
	return (
		<FixtureLibraryContext.Provider value={library}>
			{children}
		</FixtureLibraryContext.Provider>
	);
}

/** Fixture-library desk state, or null outside a mounted desk boundary. */
export function useFixtureLibrary(): FixtureLibraryState | null {
	return useContext(FixtureLibraryContext);
}
