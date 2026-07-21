import { describe, expect, it, vi } from "vitest";
import type { PlaybackActionRequest } from "../generated/light-wire";
import {
	DESK_ID,
	cueProjection,
	deskProjection,
	GROUP_ID,
	groupProjection,
} from "../../features/playbackRuntime/testFixtures";
import { PlaybackApiClient } from "./playback";
import type { LiveClientTransport } from "./transport";

const REQUEST_ID = "playback-request-1";
const SHOW_ID = "11111111-1111-4111-8111-111111111111";

const actionRequest: PlaybackActionRequest = {
	request_id: REQUEST_ID,
	address: { kind: "playback", playback_number: 1 },
	action: { type: "go", pressed: true },
	surface: "physical",
};

function actionOutcome(requestId = REQUEST_ID) {
	return {
		request_id: requestId,
		correlation_id: "55555555-5555-4555-8555-555555555555",
		requested: actionRequest.address,
		resolved: {
			kind: "playback",
			playback_number: 1,
			page: 1,
			slot: 1,
		},
		outcome: { status: "applied" },
		durability: "durable",
		projection: cueProjection(),
		related: [],
		desk: deskProjection(),
		event_sequence: 12,
		desk_event_sequence: null,
		replayed: false,
	};
}

function clientReturning(value: unknown) {
	const request = vi.fn(async (_path: string, _init?: RequestInit) => value);
	const transport = {
		request,
		blob: vi.fn(),
		absoluteUrl: vi.fn(),
		command: vi.fn(),
	} as unknown as LiveClientTransport;
	return { client: new PlaybackApiClient(transport), request };
}

describe("PlaybackApiClient v2 action boundary", () => {
	it("posts and strictly decodes an exact Group runtime snapshot", async () => {
		const identity = { kind: "group", group_id: GROUP_ID } as const;
		const { client, request } = clientReturning({
			cursor: { sequence: 11 },
			desk: deskProjection(),
			projections: [groupProjection()],
		});

		await expect(
			client.playbackRuntimeSnapshot(DESK_ID, [identity]),
		).resolves.toMatchObject({
			projections: [
				{ requested: identity, target: "group", group_id: GROUP_ID },
			],
		});
		expect(request).toHaveBeenCalledWith(
			`/api/v2/desks/${DESK_ID}/playback-runtime/snapshot`,
			expect.objectContaining({
				method: "POST",
				body: JSON.stringify({ identities: [identity] }),
			}),
		);
	});

	it("returns a decoded outcome for the submitted request", async () => {
		const { client, request } = clientReturning(actionOutcome());

		await expect(
			client.playbackRuntimeAction(SHOW_ID, DESK_ID, actionRequest),
		).resolves.toMatchObject({ request_id: REQUEST_ID });
		expect(request).toHaveBeenCalledWith(
			`/api/v2/shows/${SHOW_ID}/desks/${DESK_ID}/playback-actions`,
			expect.objectContaining({
				method: "POST",
				body: JSON.stringify(actionRequest),
			}),
		);
	});

	it("rejects a decoded outcome belonging to another request", async () => {
		const { client } = clientReturning(actionOutcome("playback-request-2"));

		await expect(
			client.playbackRuntimeAction(SHOW_ID, DESK_ID, actionRequest),
		).rejects.toMatchObject({
			name: "WireValidationError",
			path: "$.request_id",
			message: expect.stringContaining(`request ID ${REQUEST_ID}`),
		});
	});

	it("requires an existing Page for scoped desk selection", async () => {
		const { client, request } = clientReturning({
			desk_id: DESK_ID,
			page: 2,
			event_sequence: 14,
			page_creation_event_sequence: null,
		});

		await client.setPlaybackPage(DESK_ID, 2, { existingOnly: true });

		expect(request).toHaveBeenCalledWith(
			`/api/v1/control-desks/${DESK_ID}/page`,
			expect.objectContaining({
				method: "PUT",
				body: JSON.stringify({ page: 2, existing_only: true }),
			}),
		);
	});

	it("omits strict Page selection for compatibility callers", async () => {
		const { client, request } = clientReturning({
			desk_id: DESK_ID,
			page: 2,
			event_sequence: 14,
			page_creation_event_sequence: 13,
		});

		await client.setPlaybackPage(DESK_ID, 2);

		expect(request).toHaveBeenCalledWith(
			`/api/v1/control-desks/${DESK_ID}/page`,
			expect.objectContaining({
				body: JSON.stringify({ page: 2 }),
			}),
		);
	});
});
