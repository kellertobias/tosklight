import { invoke } from "@tauri-apps/api/core";
import type {
	FixtureNote,
	FixtureProfile,
	FixtureVisibility,
	PatchLayer,
	PatchSnapshot,
	VersionedObject,
} from "@tosklight/patch";

export interface DocumentSummary {
	showId: string;
	name: string;
	path: string;
	fixtureCount: number;
	fileName: string;
	lightingDesigner: string;
	showVersion: string;
	venue: string;
	contactEmail: string;
	contactPhone: string;
	project: string;
	showDate: string;
	lastSavedAt: number;
	universeCount: number;
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

/** The semantic parameters Simple mode offers. */
export type PreviewParameter = "intensity" | "pan" | "tilt" | "colour" | "gobo";

/**
 * One thing the operator set in the preview controls.
 *
 * Mirrors `viz_planning::PreviewSet`. A raw slot is addressed against the fixture's own footprint
 * rather than against a universe, so repatching moves its preview values with it.
 */
export type PreviewSet =
	| {
			kind: "semantic";
			fixture_id: string;
			parameter: PreviewParameter;
			value: number;
			/** Read for `colour` only, as red/green/blue in 0..=1. */
			colour: [number, number, number];
	  }
	| {
			kind: "slot";
			fixture_id: string;
			split: number;
			offset: number;
			value: number;
	  };

/** One ToskLight desk on the network, with the show it is running. */
export interface DeskPeer {
	instance: string;
	name: string;
	show: string | null;
	address: string;
}

export type LiveDmxProtocol = "artnet" | "sacn";
export type LiveDmxDelivery = "broadcast" | "multicast" | "unicast";

export interface LiveDmxInputMapping {
	id: string;
	logicalUniverse: number;
	protocol: LiveDmxProtocol;
	destinationUniverse: number;
	port: number;
	enabled: boolean;
	delivery: LiveDmxDelivery;
}

export interface LiveDmxInputs {
	schemaVersion: 1;
	mappings: LiveDmxInputMapping[];
}

export interface MediaTransform {
	positionMetres: [number, number, number];
	rotationDegrees: [number, number, number];
}

export interface CropRectangle {
	left: number;
	top: number;
	width: number;
	height: number;
}

export type ProjectionScreenMaterial =
	| { type: "white" }
	| { type: "grey_home_cinema" }
	| { type: "custom"; gain: number; tint_srgb: string; roughness: number };

export type MediaSectionKind =
	| {
			type: "projection_screen";
			material: ProjectionScreenMaterial;
			edge_feather: number;
	  }
	| { type: "tv"; bezel_metres: number; spill: number }
	| {
			type: "led";
			module_type_id: string;
			rows: number;
			columns: number;
			occupied_cells: number[];
	  };

export type MediaSurfaceSection = MediaSectionKind & {
	id: string;
	name: string;
	transform: MediaTransform;
	widthMetres: number;
	heightMetres: number;
	crop: CropRectangle;
};

export interface MediaServer {
	id: string;
	name: string;
	/** The patched DMX fixture that controls this server, when it was added from the library. */
	fixtureId?: string | null;
	citp: { host: string; port: number; discoveryIdentity?: string | null };
	lastKnownEndpoint?: string | null;
}

export interface MediaSource {
	id: string;
	serverId: string;
	advertisedSourceId: number;
	name: string;
	outputName?: string | null;
	width?: number | null;
	height?: number | null;
	aspectRatio?: number | null;
}

export interface LedModuleType {
	id: string;
	name: string;
	widthMetres: number;
	heightMetres: number;
	pixelPitchMillimetres: number;
	horizontalGapMetres: number;
	verticalGapMetres: number;
	pixelWidth: number;
	pixelHeight: number;
}

export interface MediaSurface {
	id: string;
	name: string;
	sourceId?: string | null;
	fallback?: {
		assetId: string;
		revision: number;
		digest: string;
		mediaType: string;
		width: number;
		height: number;
	} | null;
	sections: MediaSurfaceSection[];
}

export interface MediaProjector {
	id: string;
	name: string;
	surfaceId: string;
	transform: MediaTransform;
	bodyModel: string;
	throwRatio: number;
	lensShift: [number, number];
	coneLengthMetres: number;
	spill: number;
}

export type MediaObject =
	| {
			kind: "media_fallback_asset";
			body: {
				id: string;
				name: string;
				mediaType: string;
				digest: string;
				width: number;
				height: number;
				bytesBase64: string;
			};
	  }
	| { kind: "media_server"; body: MediaServer }
	| { kind: "media_source"; body: MediaSource }
	| { kind: "led_module_type"; body: LedModuleType }
	| { kind: "media_surface"; body: MediaSurface }
	| { kind: "media_projector"; body: MediaProjector };

export interface VersionedMediaObject {
	object: MediaObject;
	revision: number;
}

export interface MediaLayoutSnapshot {
	fallbackAssets: VersionedMediaObject[];
	servers: VersionedMediaObject[];
	sources: VersionedMediaObject[];
	ledModuleTypes: VersionedMediaObject[];
	surfaces: VersionedMediaObject[];
	projectors: VersionedMediaObject[];
}

export interface MediaLayoutOutcome {
	requestId: string;
	replayed: boolean;
	changed: boolean;
	snapshot: MediaLayoutSnapshot;
}

export interface RendererInputOverride {
	universe: number;
	protocol: "artnet" | "sacn";
}

export interface RendererSettings {
	source: "lighting_desk" | "planning_software";
	host: string;
	port: number;
	user: string;
	quality: "draft" | "standard" | "high" | "ultra" | null;
	fog: number;
	persistence: number;
	persistenceFalloff: number;
	ambient: number;
	exposure: number;
	laserBrightness: number;
	lampFogCloudiness: number;
	lampFogTurbulence: number;
	laserFogCloudiness: number;
	laserFogTurbulence: number;
	crowdAmount: number;
	theme: "light_on_dark" | "dark_on_light";
	background: [number, number, number] | null;
	showLabels: boolean;
	showSelection: boolean;
	floorGrid: boolean | null;
	blender: string;
	inputOverrides: RendererInputOverride[];
}

export type MediaObjectIntent = {
	requestId: string;
	expectedRevision: number;
	action:
		| { type: "put"; object: MediaObject }
		| { type: "delete"; kind: MediaObject["kind"]; id: string };
};

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
	/**
	 * Opens a fresh writable copy of the packaged demo show.
	 *
	 * The packaged file is a template: every call writes another copy into the operator's shows
	 * folder and opens that one, so nothing an operator does to a demo reaches the next one.
	 */
	openDemoShow: () => invoke<DocumentSummary>("open_demo_show"),
	/** Open the synchronized orthographic CAD planning window. */
	openCad: () => invoke<void>("open_cad"),
	/** Open the separate visualizer output for the document currently being edited. */
	openVisualizer: () => invoke<void>("open_visualizer"),
	/** Whether the editor-owned Visualizer child is still running. */
	visualizerIsRunning: () => invoke<boolean>("visualizer_is_running"),
	rendererSettings: () => invoke<RendererSettings>("renderer_settings"),
	saveRendererSettings: (settings: RendererSettings) =>
		invoke<RendererSettings>("save_renderer_settings", { settings }),
	/**
	 * Set one preview value.
	 *
	 * Session state of this window: it never reaches the show file and never becomes a preset or
	 * a cue. The visualizer receives it exactly as it receives a universe from the network.
	 */
	setPreview: (set: PreviewSet) => invoke<void>("set_preview", { set }),
	/** Return the named fixtures to their defaults, or every fixture when none are named. */
	clearPreview: (fixtures: readonly string[] = []) =>
		invoke<void>("clear_preview", { fixtures }),
	previewIsActive: () => invoke<boolean>("preview_is_active"),
	/**
	 * Report that the document surface is on screen.
	 *
	 * `--verify` waits for this and exits with the verdict, because a build that compiles is not
	 * evidence of a window that drew — the editor once opened white on every locally built binary.
	 */
	surfaceReady: () => invoke<void>("surface_ready"),
	current: () => invoke<DocumentSummary | null>("document_summary"),
	savePaperwork: (paperwork: {
		lightingDesigner: string;
		showVersion: string;
		venue: string;
		contactEmail: string;
		contactPhone: string;
		project: string;
		showDate: string;
	}) => invoke<DocumentSummary>("save_document_paperwork", { paperwork }),
	saveAs: (path: string) => invoke<void>("save_document_as", { path }),
	rename: (name: string) => invoke<void>("rename_document", { name }),
	exportMvr: (path: string) => invoke<number>("export_mvr", { path }),
	previewMvr: (path: string) => invoke<MvrPreview>("preview_mvr", { path }),
	importMvr: (path: string, resolutions: Record<string, MvrResolution> = {}) =>
		invoke<MvrImportReport>("import_mvr", { path, resolutions }),
	/** The desks on the network that have a show to offer. */
	discoveredDesks: () => invoke<DeskPeer[]>("discovered_desks"),
	liveDmxInputs: () => invoke<LiveDmxInputs>("live_dmx_inputs"),
	saveLiveDmxInputs: (inputs: LiveDmxInputs) =>
		invoke<LiveDmxInputs>("save_live_dmx_inputs", { inputs }),
	/** Preview compatible routes from a desk; the caller must still explicitly Apply them. */
	takeLiveDmxInputsFromDesk: (instance: string) =>
		invoke<LiveDmxInputs>("take_live_dmx_inputs_from_desk", { instance }),
	/** Take a copy of that desk's show and open it here. */
	loadFromDesk: (instance: string) =>
		invoke<DocumentSummary>("load_from_desk", { instance }),
	/** The rig as it currently stands, for surfaces outside the sheet that need the fixtures. */
	patchSnapshot: () => invoke<PatchSnapshot>("patch_snapshot"),
	mediaLayout: () => invoke<MediaLayoutSnapshot>("media_layout"),
	inspectCitpServer: (host: string, port: number) =>
		invoke<
			Array<{
				id: number;
				name: string;
				physical_output: number;
				layer: number | null;
				width: number;
				height: number;
			}>
		>("inspect_citp_server", { host, port }),
	discoverCitpServers: () =>
		invoke<Array<{ name: string; host: string; port: number }>>(
			"discover_citp_servers",
		),
	applyMediaIntent: (intent: MediaObjectIntent) =>
		invoke<MediaLayoutOutcome>("apply_media_intent", { intent }),
	importMediaFallback: (path: string) =>
		invoke<{
			reference: NonNullable<MediaSurface["fallback"]>;
			outcome: MediaLayoutOutcome;
		}>("import_media_fallback", { path }),
	patchLayers: () => invoke<PatchLayer[]>("patch_layers"),
	savePatchLayer: (layer: PatchLayer) =>
		invoke<PatchLayer>("save_patch_layer", { layer }),
	fixtureVisibility: () => invoke<FixtureVisibility[]>("fixture_visibility"),
	saveFixtureVisibility: (visibility: FixtureVisibility) =>
		invoke<FixtureVisibility>("save_fixture_visibility", { visibility }),
	fixtureNotes: () => invoke<FixtureNote[]>("fixture_notes"),
	saveFixtureNote: (note: FixtureNote) =>
		invoke<FixtureNote>("save_fixture_note", { note }),
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
