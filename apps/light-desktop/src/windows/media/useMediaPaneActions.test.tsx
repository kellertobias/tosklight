import { act, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { MediaServerFixture } from "../../api/types";
import { useMediaPaneActions } from "./useMediaPaneActions";
import { EMPTY_MEDIA_INSPECTION } from "./useMediaPaneData";

const server: MediaServerFixture = {
	fixture_id: "server-1",
	name: "Server",
	endpoint: { protocol: "citp", ip_address: "127.0.0.1", port: 4809 },
	layers: [{ fixture_id: "layer-1", head_index: 0 }],
	status: { online: false, last_success: null, last_error: "offline" },
};

function setup() {
	const batch = vi.fn().mockResolvedValue(null);
	const applySelection = vi.fn().mockResolvedValue(null);
	const input = {
		servers: [server],
		selectedServer: server,
		selectedLayer: server.layers[0],
		selectedLayerId: "layer-1",
		browserMode: "media" as const,
		sourceFilter: "media" as const,
		mainSectionId: "content",
		rightPaneVisible: false,
		inspection: EMPTY_MEDIA_INSPECTION,
		draftFolder: 2,
		applySelection,
		selectionActions: null,
		valuesActions: { batch },
		setSelectedServerId: vi.fn(),
		setSelectedLayerId: vi.fn(),
		setBrowserMode: vi.fn(),
		setSourceFilter: vi.fn(),
		setMainSectionId: vi.fn(),
		setRightPaneVisible: vi.fn(),
		setDraftFolderId: vi.fn(),
		setDraftFileId: vi.fn(),
		setInspectionError: vi.fn(),
		initializeLayer: vi.fn(),
		resetMediaData: vi.fn(),
		onPersist: vi.fn(),
	};
	const hook = renderHook(() => useMediaPaneActions(input as never));
	return { ...hook, batch, applySelection, input };
}

describe("Media pane offline actions", () => {
	it("programs an unadvertised folder and file while CITP is unavailable", () => {
		const { result, batch, applySelection } = setup();
		act(() =>
			result.current.onBrowseItem("media", {
				id: "19",
				kind: "file",
				name: "File 19",
			}),
		);
		expect(applySelection).not.toHaveBeenCalled();
		expect(batch).toHaveBeenCalledWith(
			expect.objectContaining({
				mutations: [
					expect.objectContaining({
						fixtureId: "layer-1",
						attribute: "media.folder",
					}),
					expect.objectContaining({
						fixtureId: "layer-1",
						attribute: "media.file",
					}),
				],
			}),
		);
	});

	it("programs file zero to clear content while retaining the draft folder", () => {
		const { result, batch, input } = setup();
		act(() =>
			result.current.onBrowseItem("media", {
				id: "0",
				kind: "file",
				name: "No file selected",
			}),
		);
		expect(input.setDraftFileId).toHaveBeenCalledWith("0");
		expect(batch).toHaveBeenCalledWith(
			expect.objectContaining({
				mutations: [
					expect.objectContaining({
						attribute: "media.folder",
						value: expect.objectContaining({ value: 2 / 255 }),
					}),
					expect.objectContaining({
						attribute: "media.file",
						value: expect.objectContaining({ value: 0 }),
					}),
				],
			}),
		);
	});

	it("changes source filter locally without programming a fixture", () => {
		const { result, batch, applySelection, input } = setup();
		act(() => result.current.onSelectSourceFilter("text"));
		expect(input.setSourceFilter).toHaveBeenCalledWith("text");
		expect(input.setDraftFolderId).toHaveBeenCalledWith("200");
		expect(input.setDraftFileId).toHaveBeenCalledWith(null);
		expect(input.onPersist).toHaveBeenCalledWith(
			expect.objectContaining({ sourceFilter: "text" }),
		);
		expect(batch).not.toHaveBeenCalled();
		expect(applySelection).not.toHaveBeenCalled();
	});

	it("switches the complete context to the selected server's first logical head", () => {
		const { result, input } = setup();
		const nextServer = {
			...server,
			fixture_id: "server-2",
			layers: [{ fixture_id: "server-2-layer-1", head_index: 0 }],
		};
		input.servers.push(nextServer);
		act(() => result.current.onSelectServer("server-2"));
		expect(input.setSelectedServerId).toHaveBeenCalledWith("server-2");
		expect(input.setSelectedLayerId).toHaveBeenCalledWith(
			"server-2-layer-1",
		);
		expect(input.resetMediaData).toHaveBeenCalledOnce();
		expect(input.onPersist).toHaveBeenCalledWith(
			expect.objectContaining({
				serverId: "server-2",
				layerId: "server-2-layer-1",
			}),
		);
	});
});
