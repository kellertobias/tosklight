import { describe, expect, it } from "vitest";
import {
	type BuildMediaPaneModelInput,
	buildMediaPaneModel,
} from "./buildMediaPaneModel";
import { EMPTY_MEDIA_INSPECTION } from "./useMediaPaneData";

function input(
	patch: Partial<BuildMediaPaneModelInput> = {},
): BuildMediaPaneModelInput {
	return {
		inspection: EMPTY_MEDIA_INSPECTION,
		inspectionError: null,
		servers: [],
		selectedServer: undefined,
		selectedServerId: "",
		selectedLayerId: "master",
		browserMode: "media",
		mainSectionId: "content",
		rightPaneVisible: false,
		draftFolderId: "0",
		draftFileId: null,
		thumbnailUrls: {},
		previewUrls: {},
		liveProgrammer: undefined,
		...patch,
	};
}

describe("Media pane disconnected configuration", () => {
	it("projects a truthful no-patch state without inventing a CITP problem", () => {
		const model = buildMediaPaneModel(input());
		expect(model.servers).toEqual([
			expect.objectContaining({ name: "No media server is patched" }),
		]);
		expect(model.hasPatchedServer).toBe(false);
		expect(model.hasCitpEndpoint).toBe(false);
		expect(model.showSourceFilters).toBe(false);
		expect(model.preview).toMatchObject({
			kind: "missing_patch",
			detail: "No media server is patched.",
		});
		expect(model.maskBrowser).toBe("supported");
		expect(model.libraryFolders).toHaveLength(199);
		expect(model.libraryFiles).toHaveLength(256);
		expect(model.libraryFolders[0]).toMatchObject({
			id: "1",
			name: "Folder 1",
		});
		expect(model.libraryFolders[198]).toMatchObject({
			id: "199",
			name: "Folder 199",
		});
	});

	it("keeps an unconfigured patched server and its logical layers selectable", () => {
		const server = {
			fixture_id: "server-1",
			fixture_number: 1001,
			name: "Media master",
			endpoint: null,
			layers: [{ fixture_id: "layer-1", head_index: 0 }],
			status: { online: false, last_success: null, last_error: null },
		};
		const model = buildMediaPaneModel(
			input({
				servers: [server],
				selectedServer: server,
				selectedServerId: server.fixture_id,
				selectedLayerId: "layer-1",
			}),
		);
		expect(model.servers[0]).toMatchObject({
			id: "server-1",
			fixtureLabel: "1001",
			statusLabel: "Not configured",
		});
		expect(model.hasPatchedServer).toBe(true);
		expect(model.hasCitpEndpoint).toBe(false);
		expect(model.showSourceFilters).toBe(false);
		expect(model.layers).toHaveLength(1);
		expect(model.preview).toMatchObject({
			kind: "offline",
			detail: expect.stringContaining("No CITP Media Server is available"),
		});
	});

	it("reconciles advertised names without removing unadvertised slots", () => {
		const inspection = {
			...EMPTY_MEDIA_INSPECTION,
			library_revision: "live-7",
			folders: [{ id: 7, name: "Tour package", element_count: 1 }],
			files: [
				{
					folder_id: 7,
					id: 19,
					name: "Opening loop",
					width: 1920,
					height: 1080,
					length_frames: 250,
					fps: 25,
				},
			],
		};
		const model = buildMediaPaneModel(
			input({ inspection, draftFolderId: "7" }),
		);
		expect(model.libraryFolders).toHaveLength(199);
		expect(model.libraryFolders[6]).toMatchObject({
			id: "7",
			name: "Tour package",
		});
		expect(model.libraryFolders[7]).toMatchObject({
			id: "8",
			name: "Folder 8",
		});
		expect(model.libraryFiles[19]).toMatchObject({
			id: "19",
			name: "Opening loop",
		});
		expect(model.libraryFiles[20]).toMatchObject({ id: "20", name: "File 20" });
	});

	it("projects stable visualizer and text address ranges independently", () => {
		const visualizers = buildMediaPaneModel(
			input({ sourceFilter: "visualizers" }),
		);
		expect(visualizers.libraryFolders).toHaveLength(6);
		expect(visualizers.libraryFolders.map((folder) => folder.id)).toEqual([
			"250",
			"251",
			"252",
			"253",
			"254",
			"255",
		]);
		const text = buildMediaPaneModel(input({ sourceFilter: "text" }));
		expect(text.libraryFolders).toHaveLength(50);
		expect(text.libraryFolders[0].id).toBe("200");
		expect(text.libraryFolders[49].id).toBe("249");
	});

	it("shows native source filters only for a native ToskLight server", () => {
		const baseServer = {
			fixture_id: "server-1",
			name: "Media master",
			endpoint: null,
			layers: [],
			status: { online: false, last_success: null, last_error: null },
		};
		expect(
			buildMediaPaneModel(
				input({
					servers: [baseServer],
					selectedServer: baseServer,
					selectedServerId: baseServer.fixture_id,
				}),
			).showSourceFilters,
		).toBe(false);

		const nativeServer = {
			...baseServer,
			native_action: "tosklight_media_v2",
		};
		expect(
			buildMediaPaneModel(
				input({
					servers: [nativeServer],
					selectedServer: nativeServer,
					selectedServerId: nativeServer.fixture_id,
				}),
			).showSourceFilters,
		).toBe(true);
	});

	it("projects the advertised composite dimensions into the master output", () => {
		const server = {
			fixture_id: "server-1",
			name: "Media master",
			endpoint: {
				protocol: "citp" as const,
				ip_address: "127.0.0.1",
				port: 4809,
			},
			layers: [],
			status: { online: true, last_success: null, last_error: null },
		};
		const inspection = {
			...EMPTY_MEDIA_INSPECTION,
			preview_sources: [
				{
					id: 3,
					name: "Program",
					physical_output: 0,
					layer: null,
					width: 1024,
					height: 768,
				},
			],
		};
		const model = buildMediaPaneModel(
			input({
				inspection,
				servers: [server],
				selectedServer: server,
				selectedServerId: server.fixture_id,
			}),
		);

		expect(model.preview).toMatchObject({
			kind: "ready",
			outputSize: { width: 1024, height: 768 },
		});
		expect(model.hasPatchedServer).toBe(true);
		expect(model.hasCitpEndpoint).toBe(true);
	});

	it("maps advertised isolated layer frames and failure state into the layer tile", () => {
		const server = {
			fixture_id: "server-1",
			name: "Media master",
			endpoint: {
				protocol: "citp" as const,
				ip_address: "127.0.0.1",
				port: 4809,
			},
			layers: [{ fixture_id: "layer-1", head_index: 0 }],
			status: { online: true, last_success: null, last_error: null },
		};
		const inspection = {
			...EMPTY_MEDIA_INSPECTION,
			layers: [
				{
					layer: 0,
					physical_output: 0,
					folder: 1,
					file: 2,
					name: "Damaged",
					position_frames: 0,
					length_frames: 1,
					fps: 25,
					flags: 0x8,
				},
			],
			preview_sources: [
				{
					id: 4,
					name: "Layer 1",
					physical_output: 0,
					layer: 0,
					width: 320,
					height: 180,
				},
			],
		};
		const model = buildMediaPaneModel(
			input({
				inspection,
				servers: [server],
				selectedServer: server,
				selectedServerId: server.fixture_id,
				selectedLayerId: "layer-1",
				previewUrls: { "server-1:4": "blob:live-layer" },
			}),
		);

		expect(model.layers[0]).toMatchObject({
			thumbnailSrc: "blob:live-layer",
			status: "failed",
			errorDetail: expect.stringContaining("could not render"),
		});
	});
});
