import { describe, expect, it, vi } from "vitest";
import {
	cueProjection,
	DESK_ID,
	deskProjection,
	GROUP_ID,
	groupProjection,
} from "../../features/playbackRuntime/testFixtures";
import type { PlaybackActionRequest } from "../generated/light-wire";
import type { ScreenConfiguration } from "../types";
import { PlaybackApiClient } from "./playback";
import type { LiveClientTransport } from "./transport";

const REQUEST_ID = "playback-request-1";
const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const SCREEN_ID = "22222222-2222-4222-8222-222222222222";

const screen: ScreenConfiguration = {
	id: SCREEN_ID,
	name: "Screen",
	layout: { desks: [], activeDeskId: "main" },
	content: { type: "desktop" },
	show_dock: true,
	show_playbacks: true,
	playback_count: 8,
	playback_rows: 1,
	first_playback_slot: 1,
	page_mode: "follow_main",
	show_page_controls: true,
	desired_open: false,
	display_id: null,
	bounds: null,
	fullscreen: false,
	playback_layout: null,
};

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
	const sendAction = vi.fn(async () => value);
	const transport = {
		request,
		blob: vi.fn(),
		absoluteUrl: vi.fn(),
		sendAction,
	} as unknown as LiveClientTransport;
	return {
		client: new PlaybackApiClient(transport),
		request,
		sendAction,
	};
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
			"/api/v2/playback-runtime/snapshot",
			expect.objectContaining({
				method: "POST",
				headers: expect.objectContaining({ "x-tosk-desk": DESK_ID }),
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
			"/api/v2/playback-actions",
			expect.objectContaining({
				method: "POST",
				headers: expect.objectContaining({
					"x-tosk-show": SHOW_ID,
					"x-tosk-desk": DESK_ID,
				}),
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

	it("sends a live Playback action once over the established command socket", async () => {
		const { client, sendAction, request } = clientReturning(actionOutcome());

		await expect(
			client.playbackRuntimeLiveAction(actionRequest),
		).resolves.toMatchObject({ request_id: REQUEST_ID });
		expect(sendAction).toHaveBeenCalledOnce();
		expect(sendAction).toHaveBeenCalledWith(
			{ type: "playback", request: actionRequest },
			REQUEST_ID,
		);
		expect(request).not.toHaveBeenCalled();
	});

	it("rejects a live outcome belonging to another request", async () => {
		const { client } = clientReturning(actionOutcome("playback-request-2"));

		await expect(
			client.playbackRuntimeLiveAction(actionRequest),
		).rejects.toMatchObject({
			name: "WireValidationError",
			path: "$.request_id",
			message: expect.stringContaining(`request ID ${REQUEST_ID}`),
		});
	});

	it("creates screens through the typed v2 intent boundary", async () => {
		const request = vi
			.fn()
			.mockResolvedValueOnce({ screens: [], active_pages: {} })
			.mockResolvedValueOnce({
				request_id: "screen-create",
				replayed: false,
				screen,
				active_page: null,
			});
		const client = new PlaybackApiClient({
			request,
			blob: vi.fn(),
			absoluteUrl: vi.fn(),
			sendAction: vi.fn(),
		} as unknown as LiveClientTransport);

		await expect(client.putScreen(screen)).resolves.toEqual(screen);
		expect(request).toHaveBeenNthCalledWith(1, "/api/v2/screens");
		expect(request).toHaveBeenNthCalledWith(
			2,
			"/api/v2/screens/create",
			expect.objectContaining({
				method: "POST",
				body: expect.stringContaining('"configuration"'),
			}),
		);
	});

	it("sends only changed screen fields after a v2 snapshot", async () => {
		const request = vi
			.fn()
			.mockResolvedValueOnce({ screens: [screen], active_pages: {} })
			.mockResolvedValueOnce({
				request_id: "screen-update",
				replayed: false,
				screen: { ...screen, name: "Renamed" },
				active_page: null,
			});
		const transport = {
			request,
			blob: vi.fn(),
			absoluteUrl: vi.fn(),
			sendAction: vi.fn(),
		} as unknown as LiveClientTransport;
		const client = new PlaybackApiClient(transport);
		await client.screens();
		await client.putScreen({ ...screen, name: "Renamed" });
		const body = JSON.parse(request.mock.calls[1][1].body as string);

		expect(request.mock.calls[0][0]).toBe("/api/v2/screens");
		expect(request.mock.calls[1][0]).toBe(
			`/api/v2/screens/${SCREEN_ID}/update`,
		);
		expect(body.patch).toMatchObject({
			name: "Renamed",
			show_dock: null,
		});
	});

	it("sends fixed pane content and the atomic Dock update", async () => {
		const fixed = {
			...screen,
			content: {
				type: "fixed_pane",
				pane: { type: "cues", cue_list_id: "cue-list-id" },
			},
			show_dock: false,
		} satisfies ScreenConfiguration;
		const request = vi
			.fn()
			.mockResolvedValueOnce({ screens: [screen], active_pages: {} })
			.mockResolvedValueOnce({
				request_id: "screen-fixed",
				replayed: false,
				screen: fixed,
				active_page: null,
			});
		const client = new PlaybackApiClient({
			request,
			blob: vi.fn(),
			absoluteUrl: vi.fn(),
			sendAction: vi.fn(),
		} as unknown as LiveClientTransport);

		await client.screens();
		await client.putScreen(fixed);
		const body = JSON.parse(request.mock.calls[1][1].body as string);

		expect(request.mock.calls[1][0]).toBe(
			`/api/v2/screens/${SCREEN_ID}/update`,
		);
		expect(body.patch).toMatchObject({
			content: fixed.content,
			show_dock: false,
		});
	});

	it("requires an existing Page for scoped desk selection", async () => {
		const { client, request } = clientReturning({
			request_id: "desk-page",
			replayed: false,
			desk: deskProjection(),
			page: 2,
			event_sequence: 14,
			page_creation_event_sequence: null,
		});

		await client.setPlaybackPage(DESK_ID, 2, { existingOnly: true });

		expect(request).toHaveBeenCalledWith(
			`/api/v2/control-desks/${DESK_ID}/actions`,
			expect.objectContaining({
				method: "POST",
				body: expect.stringContaining('"existing_only":true'),
			}),
		);
	});

	it("allows page creation for non-strict callers", async () => {
		const { client, request } = clientReturning({
			request_id: "desk-page-create",
			replayed: false,
			desk: deskProjection(),
			page: 2,
			event_sequence: 14,
			page_creation_event_sequence: 13,
		});

		await client.setPlaybackPage(DESK_ID, 2);

		expect(request).toHaveBeenCalledWith(
			`/api/v2/control-desks/${DESK_ID}/actions`,
			expect.objectContaining({
				body: expect.stringContaining('"existing_only":false'),
			}),
		);
	});

	it("removes a historical client through the typed replay-safe desk action", async () => {
		const { client, request } = clientReturning({
			request_id: "remove-client",
			replayed: false,
			desk: deskProjection(),
			removed: true,
			page: null,
			event_sequence: null,
			page_creation_event_sequence: null,
		});

		await client.removeClient(DESK_ID);

		expect(request).toHaveBeenCalledWith(
			`/api/v2/control-desks/${DESK_ID}/actions`,
			expect.objectContaining({
				method: "POST",
				body: expect.stringContaining('"type":"remove_client"'),
			}),
		);
	});
});
