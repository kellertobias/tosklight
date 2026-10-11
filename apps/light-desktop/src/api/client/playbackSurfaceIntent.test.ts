import { describe, it, expect, vi } from "vitest";
import { PlaybackApiClient } from "./playback";
import type { LiveClientTransport } from "./transport";
const snapshot = {
	playback_surfaces: {
		main: {
			name: "Main",
			layout: { rows: [] },
			page_source: { type: "own", page: 1 },
		},
	},
	selected_surface_id: "main",
	resolved_pages: { main: 1 },
	command_page: 1,
};
describe("surface configuration transport", () => {
	it("writes one exact surface with sparse intent and no registry replacement", async () => {
		const request = vi.fn().mockResolvedValue({
			playback_surfaces: {
				main: {
					name: "Main",
					layout: { rows: [] },
					page_source: { type: "own", page: 1 },
				},
			},
			selected_surface_id: "main",
			resolved_pages: { main: 1 },
			command_page: 1,
		});
		const client = new PlaybackApiClient({
			request,
		} as unknown as LiveClientTransport);
		await client.updatePlaybackSurface("pane:one", { name: "Renamed" });
		expect(request).toHaveBeenCalledOnce();
		const [path, init] = request.mock.calls[0];
		expect(path).toBe("/api/v2/playback-surfaces/pane%3Aone/update");
		expect(JSON.parse(init.body)).toEqual({
			request_id: expect.any(String),
			name: "Renamed",
		});
	});
	it("sends select and page as live WS actions without retry IDs or HTTP writes", async () => {
		const request = vi.fn();
		const sendAction = vi.fn().mockResolvedValue(snapshot);
		const client = new PlaybackApiClient({
			request,
			sendAction,
		} as unknown as LiveClientTransport);
		await client.selectSurface("main");
		await client.setSurfacePage("main", 99);
		expect(sendAction.mock.calls).toEqual([
			[{ type: "playback_surface_select", request: { surface_id: "main" } }],
			[
				{
					type: "playback_surface_page",
					request: { surface_id: "main", page: 99 },
				},
			],
		]);
		expect(request).not.toHaveBeenCalled();
	});
});
