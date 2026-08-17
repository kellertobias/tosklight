import { act, renderHook, waitFor } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { MediaServerFixture } from "../../api/types";
import { useMediaPaneData } from "./useMediaPaneData";

const server: MediaServerFixture = {
	fixture_id: "server-1",
	name: "Offline server",
	endpoint: { protocol: "citp", ip_address: "127.0.0.1", port: 4809 },
	layers: [{ fixture_id: "layer-1", head_index: 0 }],
	status: { online: false, last_success: null, last_error: "offline" },
};

describe("Media pane offline draft", () => {
	it("keeps the Master mask draft stable while the next inspection arrives", async () => {
		const inspection = {
			library_revision: "revision-1",
			server: { name: "Media", layer_count: 1 },
			folders: [],
			files: [],
			preview_sources: [],
			layers: [{ layer: 0, folder: 17, file: 23, flags: 0, name: "Layer 1" }],
			capabilities: {
				provider: "citp_msex" as const,
				native_action: null,
				layers: [],
			},
		};
		const inspect = vi.fn().mockResolvedValue(inspection);
		const hook = renderHook(
			({ layerId }) =>
				useMediaPaneData({
					active: true,
					server,
					layerId,
					inspect,
					refreshPreview: undefined,
					refreshThumbnails: undefined,
					loadThumbnail: undefined,
				}),
			{ initialProps: { layerId: "layer-1" } },
		);

		await waitFor(() => expect(hook.result.current.draftFolderId).toBe("17"));
		act(() => hook.result.current.initializeLayer("master"));
		hook.rerender({ layerId: "master" });
		expect(hook.result.current.draftFolderId).toBe("1");
		expect(hook.result.current.draftFileId).toBeNull();
		await waitFor(() => expect(inspect).toHaveBeenCalledTimes(2));
		expect(hook.result.current.draftFolderId).toBe("1");
		expect(hook.result.current.draftFileId).toBeNull();
		hook.unmount();
	});

	it("never reuses one folder's thumbnail for another or for an empty folder", async () => {
		const createObjectURL = vi
			.fn()
			.mockReturnValueOnce("blob:folder-1")
			.mockReturnValueOnce("blob:folder-2");
		const revokeObjectURL = vi.fn();
		vi.stubGlobal("URL", { createObjectURL, revokeObjectURL });
		const inspection = {
			library_revision: "revision-1",
			server: { name: "Media", layer_count: 1 },
			folders: [],
			files: [
				{ id: 1, folder_id: 1, name: "Media", width: 128, height: 72 },
				{ id: 1, folder_id: 2, name: "Other", width: 128, height: 72 },
			],
			preview_sources: [],
			layers: [],
			capabilities: {
				provider: "citp_msex" as const,
				native_action: null,
				layers: [],
			},
		};
		const hook = renderHook(() =>
			useMediaPaneData({
				active: true,
				server,
				layerId: "layer-1",
				inspect: vi.fn().mockResolvedValue(inspection),
				refreshPreview: undefined,
				refreshThumbnails: vi.fn().mockResolvedValue(undefined),
				loadThumbnail: vi.fn().mockResolvedValue(new Blob(["thumbnail"])),
			}),
		);

		await waitFor(() =>
			expect(hook.result.current.inspection.files).toHaveLength(2),
		);
		act(() => hook.result.current.setDraftFolderId("1"));
		await waitFor(() =>
			expect(hook.result.current.thumbnailUrls).toEqual({
				"1:1": "blob:folder-1",
			}),
		);
		act(() => hook.result.current.setDraftFolderId("2"));
		await waitFor(() =>
			expect(hook.result.current.thumbnailUrls).toEqual({
				"2:1": "blob:folder-2",
			}),
		);
		act(() => hook.result.current.setDraftFolderId("3"));
		await waitFor(() => expect(hook.result.current.thumbnailUrls).toEqual({}));
		expect(revokeObjectURL).toHaveBeenCalledWith("blob:folder-1");
		expect(revokeObjectURL).toHaveBeenCalledWith("blob:folder-2");
		hook.unmount();
		vi.unstubAllGlobals();
	});

	it("keeps locally configured slots across failed CITP polling", async () => {
		const inspect = vi.fn().mockRejectedValue(new Error("CITP unavailable"));
		const hook = renderHook(() =>
			useMediaPaneData({
				active: true,
				server,
				layerId: "layer-1",
				inspect,
				refreshPreview: undefined,
				refreshThumbnails: undefined,
				loadThumbnail: undefined,
			}),
		);

		await waitFor(() =>
			expect(hook.result.current.inspectionError).toBe("CITP unavailable"),
		);
		act(() => {
			hook.result.current.setDraftFolderId("17");
			hook.result.current.setDraftFileId("23");
		});
		await waitFor(() => expect(inspect).toHaveBeenCalledTimes(2), {
			timeout: 1_500,
		});
		expect(hook.result.current.draftFolderId).toBe("17");
		expect(hook.result.current.draftFileId).toBe("23");
		hook.unmount();
	});

	it("keeps preview polling single-flight when inspection snapshots refresh", async () => {
		vi.useFakeTimers();
		try {
			const inspect = vi.fn().mockImplementation(async () => ({
				library_revision: "revision-1",
				server: { name: "Media", layer_count: 1 },
				folders: [],
				files: [],
				preview_sources: [
					{
						id: 7,
						name: "Program",
						physical_output: 0,
						layer: null,
						width: 320,
						height: 180,
					},
				],
				layers: [],
				capabilities: {
					provider: "citp_msex",
					native_action: null,
					layers: [],
				},
			}));
			let releasePreview: () => void = () => {};
			const previewGate = new Promise<boolean>((resolve) => {
				releasePreview = () => resolve(true);
			});
			const refreshPreview = vi.fn().mockReturnValue(previewGate);
			const hook = renderHook(() =>
				useMediaPaneData({
					active: true,
					server,
					layerId: "layer-1",
					inspect: (fixtureId) => inspect(fixtureId),
					refreshPreview: (fixtureId, source) =>
						refreshPreview(fixtureId, source),
					refreshThumbnails: undefined,
					loadThumbnail: undefined,
				}),
			);

			await act(() => vi.advanceTimersByTimeAsync(3_100));
			expect(inspect.mock.calls.length).toBeGreaterThanOrEqual(3);
			expect(refreshPreview).toHaveBeenCalledTimes(1);
			releasePreview();
			await act(() => vi.advanceTimersByTimeAsync(1_100));
			expect(refreshPreview).toHaveBeenCalledTimes(2);
			hook.unmount();
		} finally {
			vi.useRealTimers();
		}
	});
});
