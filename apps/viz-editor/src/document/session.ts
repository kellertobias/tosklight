import { invoke } from "@tauri-apps/api/core";
import type { FixtureProfile, PatchLayer, VersionedObject } from "@tosklight/patch";

export interface DocumentSummary {
	showId: string;
	name: string;
	path: string;
	fixtureCount: number;
}

export interface MvrImportReport {
	importedFixtures: number;
	unresolvedFixtures: number;
	warnings: string[];
}

/** What an MVR archive holds, read before anything is written. */
export interface MvrPreview {
	fixtures: MvrPreviewFixture[];
	scenery: number;
	missingProfiles: string[];
	addressConflicts: string[];
}

export interface MvrPreviewFixture {
	uuid: string;
	name: string;
	gdtfSpec: string;
	gdtfMode: string;
	universe: number | null;
	address: number | null;
	/** Whether a profile in the library matches this fixture. */
	matched: boolean;
	/** Whether its address overlaps something already patched here. */
	conflicted: boolean;
}

/** What the operator decided about one fixture the import could not place on its own. */
export interface MvrResolution {
	action: "import" | "skip" | "import_unpatched" | "replace" | "address";
	universe?: number;
	address?: number;
}

/** One ToskLight desk on the network, with the show it is running. */
export interface DeskPeer {
	instance: string;
	name: string;
	show: string | null;
	address: string;
}

interface LibraryProfile {
	id: string;
	revision: number;
	manufacturer: string;
	name: string;
	profile: FixtureProfile;
}

export const documentSession = {
	create: (path: string, name: string) =>
		invoke<DocumentSummary>("create_document", { path, name }),
	open: (path: string) => invoke<DocumentSummary>("open_document", { path }),
	current: () => invoke<DocumentSummary | null>("document_summary"),
	saveAs: (path: string) => invoke<void>("save_document_as", { path }),
	rename: (name: string) => invoke<void>("rename_document", { name }),
	exportMvr: (path: string) => invoke<number>("export_mvr", { path }),
	previewMvr: (path: string) => invoke<MvrPreview>("preview_mvr", { path }),
	importMvr: (path: string, resolutions: Record<string, MvrResolution> = {}) =>
		invoke<MvrImportReport>("import_mvr", { path, resolutions }),
	/** The desks on the network that have a show to offer. */
	discoveredDesks: () => invoke<DeskPeer[]>("discovered_desks"),
	/** Take a copy of that desk's show and open it here. */
	loadFromDesk: (instance: string) =>
		invoke<DocumentSummary>("load_from_desk", { instance }),
	patchLayers: () => invoke<PatchLayer[]>("patch_layers"),
	savePatchLayer: (layer: PatchLayer) =>
		invoke<PatchLayer>("save_patch_layer", { layer }),
	async fixtureProfiles(): Promise<FixtureProfile[]> {
		const profiles = await invoke<LibraryProfile[]>("library_profiles");
		return profiles.map((entry) => entry.profile);
	},
};

/**
 * Patch layers as the sheet consumes them.
 *
 * They are stored objects in the document, exactly as they are on the desk: a fixture carries a
 * `layerId`, and a show whose layers lived only in a window would open on the desk with fixtures
 * pointing at layers nothing knows the names of.
 */
export function sessionPatchLayers(
	layers: readonly PatchLayer[],
): VersionedObject<PatchLayer>[] {
	return layers.map((layer) => ({
		kind: "patch_layer",
		id: layer.id,
		body: layer,
		revision: 1,
		updated_at: "",
	}));
}
