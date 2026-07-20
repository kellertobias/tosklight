import { describe, expect, it, vi } from "vitest";
import type { GroupRecordingRequest } from "../features/groupRecording/contracts";
import { HttpGroupRecordingTransport } from "./GroupRecordingTransport";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const REQUEST: GroupRecordingRequest = {
	requestId: "record-1",
	groupId: "Front Wash A / É",
	operation: "merge",
	expectedObjectRevision: 2,
};

function success() {
	return {
		status: "changed",
		request_id: REQUEST.requestId,
		correlation_id: "33333333-3333-4333-8333-333333333333",
		replayed: false,
		show_revision: 8,
		event_sequence: 12,
		group: {
			state: "stored",
			id: REQUEST.groupId,
			revision: 3,
			body: {
				id: REQUEST.groupId,
				name: "Front",
				color: null,
				icon: null,
				fixtures: ["fixture-2", "fixture-1"],
				derived_from: null,
				frozen_from: null,
				programming: {},
				master: 1,
				playback_fader: null,
			},
		},
	};
}

describe("HttpGroupRecordingTransport", () => {
	it("posts one typed action to the exact Group endpoint", async () => {
		const fetch = vi.fn(
			async (_input: RequestInfo | URL, _init?: RequestInit) =>
				new Response(JSON.stringify(success()), {
					status: 200,
					headers: {
						"content-type": "application/json",
						etag: '"3"',
					},
				}),
		);
		const transport = new HttpGroupRecordingTransport({
			baseUrl: "http://light.test/",
			sessionToken: "session-token",
			deskBoundaryToken: "desk-token",
			fetch,
		});

		const outcome = await transport.record(SHOW_ID, REQUEST);

		expect(outcome).toMatchObject({ status: "changed", eventSequence: 12 });
		expect(fetch).toHaveBeenCalledOnce();
		const [url, init] = fetch.mock.calls[0];
		expect(url).toBe(`http://light.test/api/v2/shows/${SHOW_ID}/groups/record`);
		expect(url).not.toContain("bootstrap");
		expect(init?.method).toBe("POST");
		expect(new Headers(init?.headers).get("authorization")).toBe(
			"Bearer session-token",
		);
		expect(new Headers(init?.headers).get("x-light-desk-token")).toBe(
			"desk-token",
		);
		expect(JSON.parse(String(init?.body))).toEqual({
			request_id: "record-1",
			group_id: "Front Wash A / É",
			operation: "merge",
			expected_object_revision: 2,
		});
	});

	it("reports a typed conflict without broad repair requests", async () => {
		const fetch = vi.fn(
			async () =>
				new Response(
					JSON.stringify({
						kind: "conflict",
						error: "revision conflict",
						current_revision: 7,
						retryable: false,
					}),
					{ status: 409, headers: { etag: '"7"' } },
				),
		);
		const transport = new HttpGroupRecordingTransport({
			baseUrl: "http://light.test",
			sessionToken: "session-token",
			fetch,
		});

		await expect(transport.record(SHOW_ID, REQUEST)).rejects.toEqual(
			expect.objectContaining({
				kind: "conflict",
				status: 409,
				currentRevision: 7,
				retryable: false,
			}),
		);
		expect(fetch).toHaveBeenCalledOnce();
	});

	it("classifies an ambiguous network failure as retryable", async () => {
		const transport = new HttpGroupRecordingTransport({
			baseUrl: "http://light.test",
			sessionToken: "session-token",
			fetch: vi.fn(async () => {
				throw new TypeError("connection lost");
			}),
		});

		await expect(transport.record(SHOW_ID, REQUEST)).rejects.toMatchObject({
			name: "GroupRecordingActionError",
			kind: "unavailable",
			status: 0,
			retryable: true,
		});
	});

	it("rejects malformed success payloads instead of trusting them", async () => {
		const malformed = success();
		malformed.group.id = "another-group";
		const transport = new HttpGroupRecordingTransport({
			baseUrl: "http://light.test",
			sessionToken: "session-token",
			fetch: vi.fn(
				async () =>
					new Response(JSON.stringify(malformed), {
						status: 200,
						headers: { etag: '"3"' },
					}),
			),
		});

		await expect(transport.record(SHOW_ID, REQUEST)).rejects.toThrow(
			"$.group.id",
		);
	});

	it("requires the success ETag to match the projection revision", async () => {
		const transport = new HttpGroupRecordingTransport({
			baseUrl: "http://light.test",
			sessionToken: "session-token",
			fetch: vi.fn(
				async () =>
					new Response(JSON.stringify(success()), {
						status: 200,
						headers: { etag: '"2"' },
					}),
			),
		});

		await expect(transport.record(SHOW_ID, REQUEST)).rejects.toThrow(
			"$.headers.etag",
		);
	});

	it("requires a conflict ETag to match current_revision", async () => {
		const transport = new HttpGroupRecordingTransport({
			baseUrl: "http://light.test",
			sessionToken: "session-token",
			fetch: vi.fn(
				async () =>
					new Response(
						JSON.stringify({
							kind: "conflict",
							error: "revision conflict",
							current_revision: 7,
							retryable: false,
						}),
						{ status: 409, headers: { etag: '"6"' } },
					),
			),
		});

		await expect(transport.record(SHOW_ID, REQUEST)).rejects.toThrow(
			"$.headers.etag",
		);
	});
});
