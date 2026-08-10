import { describe, expect, it, vi } from "vitest";
import type { LiveClientTransport } from "./transport";
import { MediaOutputApiClient } from "./mediaOutput";

describe("Media output advertised-resource client", () => {
	it("keeps advertised source and library IDs in typed routes", async () => {
		const request = vi.fn(async () => ({}));
		const blob = vi.fn(async () => new Blob());
		const client = new MediaOutputApiClient({
			request,
			blob,
		} as unknown as LiveClientTransport);

		await client.inspectMediaServer("master-a");
		await client.refreshMediaPreview("master-a", 9, 640, 360);
		await client.mediaPreview("master-a", 9);
		await client.refreshMediaThumbnails("master-a", 2, [7, 8], 128, 72);
		await client.mediaThumbnail("master-a", 2, 7);

		expect(request).toHaveBeenNthCalledWith(
			1,
			"/api/v2/media-servers/master-a/inspect",
		);
		expect(request).toHaveBeenNthCalledWith(
			2,
			"/api/v2/media-servers/master-a/preview/refresh",
			expect.objectContaining({
				body: JSON.stringify({ source: 9, width: 640, height: 360 }),
			}),
		);
		expect(blob).toHaveBeenNthCalledWith(
			1,
			"/api/v2/media-servers/master-a/preview/9",
		);
		expect(request).toHaveBeenNthCalledWith(
			3,
			"/api/v2/media-servers/master-a/thumbnails/refresh",
			expect.objectContaining({
				body: JSON.stringify({
					library_type: 1,
					library_level: 1,
					library_1: 2,
					library_2: 0,
					library_3: 0,
					elements: [7, 8],
					width: 128,
					height: 72,
				}),
			}),
		);
		expect(blob).toHaveBeenNthCalledWith(
			2,
			"/api/v2/media-servers/master-a/thumbnails/2/7",
		);
	});
});
