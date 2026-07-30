import { describe, expect, it, vi } from "vitest";
import type { ClientTransport } from "./transport";
import { StageLayoutApiClient } from "./stageLayout";

describe("StageLayoutApiClient", () => {
	it("posts an idempotent typed regeneration scoped to the active show", async () => {
		const request = vi.fn(async (_path: string, _init?: RequestInit) => ({
			request_id: "request-id",
			revision: 2,
			moved_fixture_ids: [],
			replayed: false,
			changed: true,
		}));
		const client = new StageLayoutApiClient({
			request,
		} as unknown as ClientTransport);
		vi.spyOn(crypto, "randomUUID").mockReturnValue(
			"00000000-0000-4000-8000-000000000026",
		);

		await client.regenerate2d("show-26", "left_to_right");

		expect(request).toHaveBeenCalledOnce();
		const [path, init] = request.mock.calls[0] ?? [];
		expect(path).toBe("/api/v2/stage-layout/actions");
		expect(init?.method).toBe("POST");
		expect(new Headers(init?.headers).get("x-tosk-show")).toBe("show-26");
		expect(JSON.parse(String(init?.body))).toEqual({
			request_id: "00000000-0000-4000-8000-000000000026",
			action: {
				type: "regenerate_2d",
				projection: "left_to_right",
			},
		});
	});
});
