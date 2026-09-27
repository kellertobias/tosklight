import { describe, expect, it, vi } from "vitest";
import { ShowApiClient } from "./shows";
import type { ClientTransport } from "./transport";

describe("ShowApiClient network catalog", () => {
	it("maps discovered show revisions into the application model", async () => {
		const request = vi.fn().mockResolvedValue({
			browsing: true,
			peers: [{
				instance: "desk-a",
				name: "Touring desk",
				address: "10.0.0.4",
				role: "desk",
				error: null,
				shows: [{
					id: "show-a",
					name: "Act One",
					updated_at: null,
					revisions: [{
						show_id: "show-a",
						revision: 2,
						name: "Opening",
						created_at: "2026-09-27T12:00:00Z",
					}],
				}],
			}],
		});
		const client = new ShowApiClient({ request } as unknown as ClientTransport);

		const catalog = await client.networkShows();
		expect(request).toHaveBeenCalledWith("/api/v2/shows/network");
		expect(catalog.peers[0].shows[0].revisions[0]).toEqual({
			show_id: "show-a",
			revision: 2,
			name: "Opening",
			created_at: "2026-09-27T12:00:00Z",
		});
	});
});
