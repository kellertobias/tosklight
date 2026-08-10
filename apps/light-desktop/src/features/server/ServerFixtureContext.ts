import type {
	FixtureAttributeMapping,
	FixturePackageImportOutcome,
	FixtureSourceMapping,
	GelCatalog,
	GelCatalogImportPreview,
	GelCatalogImportTarget,
} from "../../api/client/fixtures";
import type {
	FixtureDefinition,
	FixtureProfile,
	PatchLayer,
} from "../../api/types";
import type { MediaServerInspection } from "../../api/client/mediaOutput";

export interface ServerFixtureContext {
	refreshMediaPreview: (fixtureId: string, source?: number) => Promise<boolean>;
	refreshMediaThumbnails: (
		fixtureId: string,
		folder: number,
		elements: number[],
	) => Promise<void>;
	inspectMediaServer: (fixtureId: string) => Promise<MediaServerInspection>;
	mediaThumbnail: (
		fixtureId: string,
		folder: number,
		element: number,
	) => Promise<Blob>;
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
	fixtureSourceMappings: () => Promise<FixtureSourceMapping[]>;
	rememberFixtureSourceMapping: (input: {
		sourceFormat: string;
		sourceAttribute: string;
		targetAttribute: string | null;
	}) => Promise<FixtureSourceMapping | null>;
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
