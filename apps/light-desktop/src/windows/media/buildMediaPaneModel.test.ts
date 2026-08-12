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
	it("keeps all Content and Mask slots visible without a CITP server", () => {
		const model = buildMediaPaneModel(input());
		expect(model.servers).toEqual([
			expect.objectContaining({ name: "No CITP Media Server available" }),
		]);
		expect(model.preview).toMatchObject({
			kind: "missing_patch",
			detail: expect.stringContaining("No CITP Media Server is available"),
		});
		expect(model.maskBrowser).toBe("supported");
		expect(model.libraryFolders).toHaveLength(256);
		expect(model.libraryFiles).toHaveLength(256);
		expect(model.libraryFolders[0]).toMatchObject({
			id: "0",
			name: "Folder 0",
		});
		expect(model.libraryFolders[255]).toMatchObject({
			id: "255",
			name: "Folder 255",
		});
	});

	it("keeps an unconfigured patched server and its logical layers selectable", () => {
		const server = {
			fixture_id: "server-1",
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
			statusLabel: "Not configured",
		});
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
		expect(model.libraryFolders).toHaveLength(256);
		expect(model.libraryFolders[7]).toMatchObject({
			id: "7",
			name: "Tour package",
		});
		expect(model.libraryFolders[8]).toMatchObject({
			id: "8",
			name: "Folder 8",
		});
		expect(model.libraryFiles[19]).toMatchObject({
			id: "19",
			name: "Opening loop",
		});
		expect(model.libraryFiles[20]).toMatchObject({ id: "20", name: "File 20" });
	});
});
