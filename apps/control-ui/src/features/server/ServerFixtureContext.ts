import type {
	FixtureDefinition,
	FixtureProfile,
	PatchedFixture,
	PatchLayer,
} from "../../api/types";

export interface ServerFixtureContext {
	refreshMediaPreview: (fixtureId: string, source?: number) => Promise<boolean>;
	refreshMediaThumbnails: (
		fixtureId: string,
		elements: number[],
	) => Promise<void>;
	configureMediaServer: (
		fixtureId: string,
		ipAddress: string | null,
		port?: number,
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
	patchFixture: (input: {
		name: string;
		fixture_number: number | null;
		virtual_fixture_number?: number | null;
		definition: FixtureDefinition;
		universe: number | null;
		address: number | null;
		split_patches?: import("../../api/types").SplitPatch[];
		layer_id?: string;
	}) => Promise<string | null>;
	updatePatchedFixture: (
		fixtureId: string,
		changes: Partial<PatchedFixture>,
	) => Promise<boolean>;
	deletePatchedFixture: (fixtureId: string) => Promise<boolean>;
	savePatchLayer: (layer: PatchLayer) => Promise<boolean>;
}
