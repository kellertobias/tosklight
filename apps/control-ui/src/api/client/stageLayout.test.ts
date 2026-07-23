import { describe, expect, it, vi } from "vitest";
import type { StageLayoutActionRequest } from "../generated/light-wire";
import { StageLayoutApiClient } from "./stageLayout";
import type { ClientTransport } from "./transport";

const REQUEST_ID = "stage-layout-request-1";

const actionRequest: StageLayoutActionRequest = {
	request_id: REQUEST_ID,
	action: {
		type: "move_selection",
		fixture_ids: ["fixture-b", "fixture-a"],
		axis: "rotation_z",
		delta: 12.5,
	},
};

function clientReturning(value: unknown) {
	const request = vi.fn(async (_path: string, _init?: RequestInit) => value);
	const transport = {
		request,
		blob: vi.fn(),
		absoluteUrl: vi.fn(),
	} as unknown as ClientTransport;
	return { client: new StageLayoutApiClient(transport), request };
}

describe("StageLayoutApiClient v2 action boundary", () => {
	it("posts the move-selection intent to the active-show action route", async () => {
		const outcome = {
			request_id: REQUEST_ID,
			revision: 4,
			moved_fixture_ids: ["fixture-b", "fixture-a"],
			replayed: false,
			changed: true,
		};
		const { client, request } = clientReturning(outcome);

		await expect(client.moveSelection(actionRequest)).resolves.toEqual(outcome);
		expect(request).toHaveBeenCalledTimes(1);
		expect(request).toHaveBeenCalledWith(
			"/api/v2/stage-layout/actions",
			expect.objectContaining({
				method: "POST",
				body: JSON.stringify(actionRequest),
			}),
		);
	});
});
