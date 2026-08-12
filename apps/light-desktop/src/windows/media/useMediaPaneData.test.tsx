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
});
