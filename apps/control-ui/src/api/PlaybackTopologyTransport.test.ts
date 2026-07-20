import { describe, expect, it, vi } from "vitest";
import type { PlaybackTopologyRequest } from "../features/playbackTopology/contracts";
import {
	HttpPlaybackTopologyTransport,
	PlaybackTopologyActionError,
} from "./PlaybackTopologyTransport";
import { WireValidationError } from "./wireValidation";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const REQUEST_ID = "22222222-2222-4222-8222-222222222222";

function request(): PlaybackTopologyRequest {
	return {
		requestId: REQUEST_ID,
		action: {
			type: "clear_mapped_playback",
			page: 1,
			slot: 2,
			expectedPageRevision: 3,
			expectedPageObjectId: "1",
			expectedPlaybackRevision: 4,
			expectedPlaybackObjectId: "7",
		},
	};
}

function outcome() {
	return {
		request_id: REQUEST_ID,
		correlation_id: "33333333-3333-4333-8333-333333333333",
		show_revision: 8,
		resolution: {
			kind: "page_slot",
			page: 1,
			slot: 2,
			playback_number: 7,
		},
		status: "changed",
		objects: [
			{
				state: "present",
				kind: "playback_page",
				object_id: "1",
				object_revision: 4,
				body: { number: 1, name: "Page 1", slots: {} },
			},
			{
				state: "deleted",
				kind: "playback",
				object_id: "7",
				object_revision: 5,
			},
		],
		event_sequence: 19,
		replayed: false,
	};
}

function response(value: unknown, status: number, etag?: string) {
	return new Response(JSON.stringify(value), {
		status,
		headers: etag ? { etag } : undefined,
	});
}

describe("Playback topology v2 HTTP adapter", () => {
	it("is dormant until one authenticated, revision-checked action", async () => {
		const fetchMock = vi.fn(
			async (_input: RequestInfo | URL, _init?: RequestInit) =>
				response(outcome(), 200, '"8"'),
		);
		const transport = new HttpPlaybackTopologyTransport({
			baseUrl: "http://desk.local/",
			sessionToken: "session-token",
			deskBoundaryToken: "desk-token",
			fetch: fetchMock as typeof fetch,
		});
		expect(fetchMock).not.toHaveBeenCalled();

		await expect(transport.apply(SHOW_ID, 7, request())).resolves.toMatchObject({
			status: "changed",
			showRevision: 8,
		});

		const [url, init] = fetchMock.mock.calls[0];
		expect(String(url)).toBe(
			`http://desk.local/api/v2/shows/${SHOW_ID}/playback-topology/actions`,
		);
		const headers = init?.headers as Headers;
		expect(headers.get("authorization")).toBe("Bearer session-token");
		expect(headers.get("x-light-desk-token")).toBe("desk-token");
		expect(headers.get("if-match")).toBe('"7"');
		expect(JSON.parse(String(init?.body))).toEqual({
			request_id: REQUEST_ID,
			action: {
				type: "clear_mapped_playback",
				page: 1,
				slot: 2,
				expected_page_revision: 3,
				expected_playback_revision: 4,
			},
		});
		expect(String(url)).not.toMatch(/bootstrap|api\/v1\/playbacks/);
	});

	it("surfaces exact conflict and related revisions", async () => {
		const transport = new HttpPlaybackTopologyTransport({
			baseUrl: "http://desk.local",
			sessionToken: "session-token",
			fetch: vi.fn(async () =>
				response(
					{
						kind: "conflict",
						error: "stale Playback Page revision",
						current_revision: 9,
						current_related_revision: 6,
						retryable: false,
					},
					409,
					'"9"',
				),
			) as typeof fetch,
		});

		await expect(transport.apply(SHOW_ID, 7, request())).rejects.toMatchObject({
			name: "PlaybackTopologyActionError",
			status: 409,
			currentRevision: 9,
			currentRelatedRevision: 6,
			retryable: false,
		});
	});

	it("rejects missing or mismatched authoritative ETags", async () => {
		for (const result of [
			response(outcome(), 200),
			response(outcome(), 200, '"7"'),
		]) {
			const transport = new HttpPlaybackTopologyTransport({
				baseUrl: "http://desk.local",
				sessionToken: "session-token",
				fetch: vi.fn(async () => result) as typeof fetch,
			});
			await expect(transport.apply(SHOW_ID, 7, request())).rejects.toBeInstanceOf(
				WireValidationError,
			);
		}
	});

	it("marks a network failure retryable without issuing an implicit replay", async () => {
		const fetchMock = vi.fn(async () => {
			throw new TypeError("offline");
		});
		const transport = new HttpPlaybackTopologyTransport({
			baseUrl: "http://desk.local",
			sessionToken: "session-token",
			fetch: fetchMock as typeof fetch,
		});
		await expect(transport.apply(SHOW_ID, 7, request())).rejects.toEqual(
			expect.objectContaining({
				name: "PlaybackTopologyActionError",
				status: 0,
				retryable: true,
			}),
		);
		expect(fetchMock).toHaveBeenCalledOnce();
		expect(PlaybackTopologyActionError).toBeDefined();
	});
});
