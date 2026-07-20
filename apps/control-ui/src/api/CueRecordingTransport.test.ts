import { describe, expect, it, vi } from "vitest";
import type { CueRecordingRequest } from "../features/cueRecording/contracts";
import {
	CueRecordingActionError,
	HttpCueRecordingTransport,
} from "./CueRecordingTransport";
import { WireValidationError } from "./wireValidation";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const CUE_LIST_ID = "22222222-2222-4222-8222-222222222222";
const CUE_ID = "33333333-3333-4333-8333-333333333333";
const REQUEST_ID = "44444444-4444-4444-8444-444444444444";

function request(): CueRecordingRequest {
	return {
		requestId: REQUEST_ID,
		target: { kind: "cue_list", cueListId: CUE_LIST_ID },
		operation: "merge",
		cueNumber: 1,
		timing: { fadeMillis: 1000 },
		cueOnly: true,
		capturePolicy: "current_capture",
		activationPolicy: "hold",
	};
}

function changedOutcome() {
	return {
		status: "changed",
		request_id: REQUEST_ID,
		correlation_id: "55555555-5555-4555-8555-555555555555",
		replayed: false,
		captured_source: "normal",
		show_revision: 8,
		recorded_cue: { id: CUE_ID, number: 1, deleted: false },
		projections: {
			cue_list: {
				id: CUE_LIST_ID,
				revision: 2,
				body: {
					id: CUE_LIST_ID,
					name: "Main",
					priority: 0,
					mode: "sequence",
					looped: false,
					cues: [
						{
							id: CUE_ID,
							number: 1,
							name: "Opening",
							fade_millis: 1000,
							delay_millis: 0,
							trigger: { type: "manual" },
							cue_only: true,
							changes: [],
							group_changes: [],
							phasers: [],
						},
					],
				},
			},
			playback: null,
			page: null,
		},
		show_event_sequence: 12,
		runtime: null,
	};
}

function jsonResponse(
	value: unknown,
	status: number,
	etag?: string,
) {
	return new Response(JSON.stringify(value), {
		status,
		headers: {
			"content-type": "application/json",
			...(etag ? { etag } : {}),
		},
	});
}

describe("Cue recording v2 HTTP adapter", () => {
	it("is dormant until one scoped action and sends auth, desk, revision, and strict body", async () => {
		const fetchMock = vi.fn(
			async (_input: RequestInfo | URL, _init?: RequestInit) =>
				jsonResponse(changedOutcome(), 200, '"8"'),
		);
		const transport = new HttpCueRecordingTransport({
			baseUrl: "http://desk.local/",
			sessionToken: "session-token",
			deskBoundaryToken: "desk-token",
			fetch: fetchMock as typeof fetch,
		});
		expect(fetchMock).not.toHaveBeenCalled();

		await expect(transport.record(SHOW_ID, 7, request())).resolves.toMatchObject({
			status: "changed",
			showRevision: 8,
		});

		expect(fetchMock).toHaveBeenCalledOnce();
		const [url, init] = fetchMock.mock.calls[0];
		expect(String(url)).toBe(
			`http://desk.local/api/v2/shows/${SHOW_ID}/cues/record`,
		);
		expect(init?.method).toBe("POST");
		const headers = init?.headers as Headers;
		expect(headers.get("authorization")).toBe("Bearer session-token");
		expect(headers.get("x-light-desk-token")).toBe("desk-token");
		expect(headers.get("if-match")).toBe('"7"');
		expect(headers.get("content-type")).toBe("application/json");
		expect(JSON.parse(String(init?.body))).toEqual({
			request_id: REQUEST_ID,
			target: { kind: "cue_list", cue_list_id: CUE_LIST_ID },
			operation: "merge",
			cue_number: 1,
			timing: { fade_millis: 1000 },
			cue_only: true,
			name: null,
			capture_policy: "current_capture",
			activation_policy: "hold",
		});
		expect(String(url)).not.toMatch(/bootstrap|programmers|selection/);
	});

	it("omits the desk boundary header when the current desk has no token", async () => {
		const fetchMock = vi.fn(
			async (_input: RequestInfo | URL, _init?: RequestInit) =>
				jsonResponse(changedOutcome(), 200, '"8"'),
		);
		const transport = new HttpCueRecordingTransport({
			baseUrl: "http://desk.local",
			sessionToken: "session-token",
			fetch: fetchMock as typeof fetch,
		});

		await transport.record(SHOW_ID, 7, request());

		const headers = fetchMock.mock.calls[0][1]?.headers as Headers;
		expect(headers.has("x-light-desk-token")).toBe(false);
	});

	it("surfaces a strict conflict with the authoritative ETag revision", async () => {
		const transport = new HttpCueRecordingTransport({
			baseUrl: "http://desk.local",
			sessionToken: "session-token",
			fetch: vi.fn(async () =>
				jsonResponse(
					{
						kind: "conflict",
						error: "revision conflict",
						current_revision: 9,
						retryable: false,
					},
					409,
					'"9"',
				),
			) as typeof fetch,
		});

		await expect(transport.record(SHOW_ID, 7, request())).rejects.toMatchObject({
			name: "CueRecordingActionError",
			kind: "conflict",
			status: 409,
			currentRevision: 9,
			retryable: false,
		});
	});

	it("rejects missing or mismatched success and conflict ETags", async () => {
		for (const response of [
			jsonResponse(changedOutcome(), 200),
			jsonResponse(changedOutcome(), 200, '"7"'),
			jsonResponse(
				{
					kind: "conflict",
					error: "revision conflict",
					current_revision: 9,
					retryable: false,
				},
				409,
				'"8"',
			),
		]) {
			const transport = new HttpCueRecordingTransport({
				baseUrl: "http://desk.local",
				sessionToken: "session-token",
				fetch: vi.fn(async () => response) as typeof fetch,
			});
			await expect(transport.record(SHOW_ID, 7, request())).rejects.toBeInstanceOf(
				WireValidationError,
			);
		}
	});

	it("maps transport failure and malformed server errors without replay ambiguity", async () => {
		const offline = new HttpCueRecordingTransport({
			baseUrl: "http://desk.local",
			sessionToken: "session-token",
			fetch: vi.fn(async () => {
				throw new TypeError("network offline");
			}) as typeof fetch,
		});
		await expect(offline.record(SHOW_ID, 7, request())).rejects.toEqual(
			expect.objectContaining({
				name: "CueRecordingActionError",
				kind: "unavailable",
				status: 0,
				currentRevision: null,
				retryable: true,
			}),
		);

		const malformed = new HttpCueRecordingTransport({
			baseUrl: "http://desk.local",
			sessionToken: "session-token",
			fetch: vi.fn(async () => new Response("gateway broke", { status: 502 })) as
				typeof fetch,
		});
		await expect(malformed.record(SHOW_ID, 7, request())).rejects.toEqual(
			expect.objectContaining({
				name: "CueRecordingActionError",
				kind: "internal",
				status: 502,
				retryable: true,
			}),
		);
	});

	it("keeps structured authorization rejection non-retryable", async () => {
		const transport = new HttpCueRecordingTransport({
			baseUrl: "http://desk.local",
			sessionToken: "foreign-session",
			fetch: vi.fn(async () =>
				jsonResponse(
					{
						kind: "forbidden",
						error: "foreign user",
						retryable: false,
					},
					403,
				),
			) as typeof fetch,
		});

		await expect(transport.record(SHOW_ID, 7, request())).rejects.toEqual(
			expect.objectContaining({
				name: "CueRecordingActionError",
				kind: "forbidden",
				status: 403,
				retryable: false,
			}),
		);
	});

	it("exports the typed failure shape used for replay decisions", () => {
		const failure = new CueRecordingActionError(
			"connection lost",
			"unavailable",
			0,
			null,
			true,
		);
		expect(failure).toMatchObject({
			name: "CueRecordingActionError",
			kind: "unavailable",
			status: 0,
			currentRevision: null,
			retryable: true,
		});
	});
});
