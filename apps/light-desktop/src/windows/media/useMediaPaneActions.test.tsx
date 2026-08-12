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
		selectedServer: server,
		selectedLayer: server.layers[0],
		selectedLayerId: "layer-1",
		browserMode: "media" as const,
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
		setMainSectionId: vi.fn(),
		setRightPaneVisible: vi.fn(),
		setDraftFolderId: vi.fn(),
		setDraftFileId: vi.fn(),
		setInspectionError: vi.fn(),
		initializeLayer: vi.fn(),
		resetMediaData: vi.fn(),
	};
	const hook = renderHook(() => useMediaPaneActions(input as never));
	return { ...hook, batch, applySelection };
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
});
