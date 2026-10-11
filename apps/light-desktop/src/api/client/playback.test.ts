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

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const SCREEN_ID = "22222222-2222-4222-8222-222222222222";
const CLIENT_ID = "33333333-3333-4333-8333-333333333333";

const screen: ScreenConfiguration = {
	id: SCREEN_ID,
	name: "Screen",
	layout: { desks: [], activeDeskId: "main" },
	content: { type: "desktop" },
	show_dock: true,
	show_playbacks: true,
	surface_id: "screen-main",
	show_page_controls: true,
	show_programmer: false,
	not_editable: false,
	desired_open: false,
	display_id: null,
	bounds: null,
	fullscreen: false,
};

const actionRequest: PlaybackActionRequest = {
	address: { kind: "playback", page: 1, position: 1 },
	action: { type: "go", pressed: true },
	control_source: { surface_id: "main", control_id: 11 },
};

function actionOutcome() {
	return {
		correlation_id: "55555555-5555-4555-8555-555555555555",
		requested: actionRequest.address,
		resolved: { kind: "playback", page: 1, position: 1 },
		outcome: { status: "applied" },
		durability: "durable",
		projection: cueProjection(),
		related: [],
		desk: deskProjection(),
		event_sequence: 12,
		desk_event_sequence: null,
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
			projections: [{ identity, target: "group", group_id: GROUP_ID }],
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
		).resolves.toMatchObject({ resolved: actionRequest.address });
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

	it("rejects malformed resolved addresses before returning an HTTP outcome", async () => {
		const { client } = clientReturning({
			...actionOutcome(),
			resolved: { kind: "playback", page: 1, position: 1001 },
		});
		await expect(
			client.playbackRuntimeAction(SHOW_ID, DESK_ID, actionRequest),
		).rejects.toMatchObject({
			name: "WireValidationError",
			path: "$.resolved.position",
		});
	});

	it("sends a live Playback action once over the established command socket", async () => {
		const { client, sendAction, request } = clientReturning(actionOutcome());

		await expect(
			client.playbackRuntimeLiveAction(actionRequest),
		).resolves.toMatchObject({ resolved: actionRequest.address });
		expect(sendAction).toHaveBeenCalledOnce();
		expect(sendAction).toHaveBeenCalledWith({
			type: "playback",
			request: actionRequest,
		});
		expect(request).not.toHaveBeenCalled();
	});

	it("rejects malformed live outcome addresses without retrying", async () => {
		const { client, sendAction, request } = clientReturning({
			...actionOutcome(),
			resolved: { kind: "playback", page: 0, position: 1 },
		});
		await expect(
			client.playbackRuntimeLiveAction(actionRequest),
		).rejects.toMatchObject({
			name: "WireValidationError",
			path: "$.resolved.page",
		});
		expect(sendAction).toHaveBeenCalledOnce();
		expect(request).not.toHaveBeenCalled();
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

	it("updates the programmer owner and encoder count as one replay-safe screen intent", async () => {
		const request = vi.fn().mockResolvedValueOnce({
			request_id: "programmer-control-surface",
			replayed: false,
			screen: null,
			active_page: null,
			programmer_control_surface: {
				owner_screen_id: SCREEN_ID,
				visible_encoders: 4,
			},
		});
		const client = new PlaybackApiClient({
			request,
			blob: vi.fn(),
			absoluteUrl: vi.fn(),
			sendAction: vi.fn(),
		} as unknown as LiveClientTransport);

		await expect(
			client.updateProgrammerControlSurface({
				owner_screen_id: SCREEN_ID,
				visible_encoders: 4,
			}),
		).resolves.toEqual({ owner_screen_id: SCREEN_ID, visible_encoders: 4 });
		expect(request).toHaveBeenCalledWith(
			"/api/v2/screens/actions",
			expect.objectContaining({
				method: "POST",
				body: expect.stringContaining(
					'"type":"update_programmer_control_surface"',
				),
			}),
		);
		const body = JSON.parse(request.mock.calls[0][1].body as string);
		expect(body.action.patch).toEqual({
			owner_screen_id: SCREEN_ID,
			assign_to_main: false,
			visible_encoders: 4,
		});
	});

	it("changes exactly the addressed surface page over the established live socket", async () => {
		const snapshot = {
			playback_surfaces: {
				main: {
					name: "Main",
					layout: { rows: [] },
					page_source: { type: "own", page: 1 },
				},
				"screen-main": {
					name: "Screen",
					layout: { rows: [] },
					page_source: { type: "own", page: 99 },
				},
			},
			selected_surface_id: "main",
			resolved_pages: { main: 1, "screen-main": 99 },
			command_page: 1,
		};
		const { client, request, sendAction } = clientReturning(snapshot);
		await expect(client.setSurfacePage("screen-main", 99)).resolves.toEqual(
			snapshot,
		);
		expect(sendAction).toHaveBeenCalledExactlyOnceWith({
			type: "playback_surface_page",
			request: { surface_id: "screen-main", page: 99 },
		});
		expect(request).not.toHaveBeenCalled();
	});
	it("rejects invalid authoritative surface page state without an HTTP fallback", async () => {
		const { client, request, sendAction } = clientReturning({
			playback_surfaces: {},
			selected_surface_id: "main",
			resolved_pages: { main: 100 },
			command_page: 100,
		});
		await expect(client.setSurfacePage("main", 99)).rejects.toMatchObject({
			name: "WireValidationError",
		});
		expect(sendAction).toHaveBeenCalledOnce();
		expect(request).not.toHaveBeenCalled();
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

		await client.removeClient(DESK_ID, CLIENT_ID);

		expect(request).toHaveBeenCalledWith(
			`/api/v2/control-desk/actions`,
			expect.objectContaining({
				method: "POST",
				body: expect.stringContaining(`"client_id":"${CLIENT_ID}"`),
			}),
		);
	});
});
