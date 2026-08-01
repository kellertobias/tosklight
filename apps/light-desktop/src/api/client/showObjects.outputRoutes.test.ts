import { afterEach, describe, expect, it, vi } from "vitest";
import { ShowObjectsApiClient } from "./showObjects";
import type { ClientTransport } from "./transport";

afterEach(() => vi.restoreAllMocks());

describe("ShowObjectsApiClient output route ranges", () => {
	it("sends one paired range intent with independent replay and range identities", async () => {
		const request = vi.fn(async (_path: string, _init?: RequestInit) => ({
			request_id: "request-id",
			replayed: false,
			changes: [],
			event_sequence: 0,
		}));
		const client = new ShowObjectsApiClient({
			request,
		} as unknown as ClientTransport);
		vi.spyOn(crypto, "randomUUID")
			.mockReturnValueOnce("00000000-0000-4000-8000-000000000080")
			.mockReturnValueOnce("00000000-0000-4000-8000-000000000081");

		await client.createOutputRouteRange("show-1", {
			logical_start: 1,
			logical_end: 8,
			destination_start: 101,
			destination_end: 108,
			route: {
				protocol: "art_net",
				delivery_mode: "broadcast",
				destination: null,
				enabled: true,
				minimum_slots: 128,
			},
		});

		expect(request).toHaveBeenCalledOnce();
		const [path, init] = request.mock.calls[0] ?? [];
		expect(path).toBe("/api/v2/output-routes/actions");
		expect(new Headers(init?.headers).get("x-tosk-show")).toBe("show-1");
		expect(JSON.parse(String(init?.body))).toEqual({
			request_id: "00000000-0000-4000-8000-000000000081",
			action: {
				type: "create_range",
				range_id: "00000000-0000-4000-8000-000000000080",
				route: {
					protocol: "art_net",
					logical_universe: 1,
					destination_universe: 101,
					delivery_mode: "broadcast",
					destination: null,
					enabled: true,
					minimum_slots: 128,
				},
				logical_universe_end: 8,
				destination_universe_end: 108,
			},
		});
	});
});
