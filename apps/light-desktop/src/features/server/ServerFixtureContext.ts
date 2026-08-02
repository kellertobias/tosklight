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
} from "../../api/types";

export interface ServerFixtureContext {
	refreshMediaPreview: (fixtureId: string, source?: number) => Promise<boolean>;
	refreshMediaThumbnails: (
		fixtureId: string,
		elements: number[],
	) => Promise<void>;
	saveFixtureDefinition: (definition: FixtureDefinition) => Promise<boolean>;
	deleteFixtureDefinition: (id: string, revision: number) => Promise<void>;
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
	gelCatalogs: (query?: string) => Promise<GelCatalog[]>;
	previewGelCatalogCsvImport: (input: {
		target: GelCatalogImportTarget;
		catalogName: string;
		csv: Uint8Array;
	}) => Promise<GelCatalogImportPreview>;
	confirmGelCatalogCsvImport: (input: {
		target: GelCatalogImportTarget;
		catalogName: string;
		csv: Uint8Array;
	}) => Promise<GelCatalog>;
	savePatchLayer: (layer: PatchLayer) => Promise<boolean>;
}
