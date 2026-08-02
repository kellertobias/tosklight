import { describe, expect, it, vi } from "vitest";
import { ShowObjectsApiClient } from "./showObjects";
import type { ClientTransport } from "./transport";

describe("ShowObjectsApiClient Dynamic spatial preview", () => {
	it("previews an unsaved draft against explicit Dynamic and show revisions", async () => {
		const request = vi.fn(async (_path: string, _init?: RequestInit) => ({
			ordered_fixture_ids: [],
		}));
		const client = new ShowObjectsApiClient({
			request,
		} as unknown as ClientTransport);
		const spatial_mapping = {
			projection: { type: "inherit" as const },
			shape: { type: "inherit" as const },
		};

		await client.previewDynamicSpatialMapping("show-1", "dynamic/1", {
			expected_dynamic_revision: 7,
			expected_show_revision: 19,
			spatial_mapping,
		});

		const [path, init] = request.mock.calls[0] ?? [];
		expect(path).toBe("/api/v2/dynamics/dynamic%2F1/spatial-preview");
		expect(new Headers(init?.headers).get("x-tosk-show")).toBe("show-1");
		expect(JSON.parse(String(init?.body))).toEqual({
			expected_dynamic_revision: 7,
			expected_show_revision: 19,
			spatial_mapping,
		});
	});
});
