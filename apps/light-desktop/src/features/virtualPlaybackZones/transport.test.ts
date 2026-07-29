import { describe, expect, it, vi } from "vitest";
import { HttpVirtualPlaybackZonesTransport } from "./transport";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const SCOPE = { showId: SHOW_ID };
const ZONES = [
	{ id: "paired", name: "Paired", playbackNumbers: [1001, 1301] },
] as const;

function json(value: unknown, status = 200) {
	return new Response(JSON.stringify(value), {
		status,
		headers: { "content-type": "application/json" },
	});
}

function transport(fetchImplementation: typeof globalThis.fetch) {
	return new HttpVirtualPlaybackZonesTransport({
		baseUrl: "http://127.0.0.1:5000/",
		sessionToken: "session-token",
		deskBoundaryToken: "desk-boundary",
		fetch: fetchImplementation,
	});
}

describe("HttpVirtualPlaybackZonesTransport", () => {
	it("loads a show-global snapshot without a desk header", async () => {
		const request = vi
			.fn<typeof globalThis.fetch>()
			.mockResolvedValueOnce(
				json({ show_id: SHOW_ID, revision: 4, zones: [] }),
			);
		await expect(transport(request).loadSnapshot(SCOPE)).resolves.toMatchObject(
			{
				showId: SHOW_ID,
				revision: 4,
			},
		);
		const [url, init] = request.mock.calls[0];
		expect(url).toBe(
			"http://127.0.0.1:5000/api/v2/virtual-playback-exclusion-zones",
		);
		const headers = new Headers(init?.headers);
		expect(headers.get("x-tosk-show")).toBe(SHOW_ID);
		expect(headers.get("x-tosk-desk")).toBeNull();
	});

	it("posts one retry-safe show-global update", async () => {
		const request = vi.fn<typeof globalThis.fetch>().mockResolvedValueOnce(
			json({
				show_id: SHOW_ID,
				revision: 5,
				zones: [
					{
						id: "paired",
						name: "Paired",
						playback_numbers: [1001, 1301],
					},
				],
				request_id: "request-a",
				replayed: false,
				changed: true,
			}),
		);
		await expect(
			transport(request).save(SCOPE, 4, ZONES, "request-a"),
		).resolves.toMatchObject({
			revision: 5,
			zones: [{ playbackNumbers: [1001, 1301] }],
		});
		const [url, init] = request.mock.calls[0];
		expect(url).toBe(
			"http://127.0.0.1:5000/api/v2/virtual-playback-exclusion-zones/update",
		);
		expect(init?.method).toBe("POST");
		expect(JSON.parse(String(init?.body))).toEqual({
			request_id: "request-a",
			expected_revision: 4,
			zones: [
				{
					id: "paired",
					name: "Paired",
					playback_numbers: [1001, 1301],
				},
			],
		});
	});
});
