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
	importFixturePackage: (source: Uint8Array) => Promise<FixtureProfile>;
	exportFixturePackage: (id: string, revision: number) => Promise<Blob>;
	savePatchLayer: (layer: PatchLayer) => Promise<boolean>;
}
