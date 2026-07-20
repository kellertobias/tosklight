import { describe, expect, it, vi } from "vitest";
import { HttpPresetRecordingTransport } from "./PresetRecordingTransport";
import {
	decodePresetRecordErrorResponse,
	decodePresetRecordingOutcome,
	encodePresetRecordingRequest,
} from "./presetRecordingWire";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const REQUEST_ID = "22222222-2222-4222-8222-222222222222";
const CORRELATION_ID = "33333333-3333-4333-8333-333333333333";

function request() {
	return {
		requestId: REQUEST_ID,
		address: { family: "Color" as const, number: 7 },
		name: "Deep Blue",
		mode: "merge" as const,
		expectedObjectRevision: 4,
	};
}

function wirePreset(name = "Deep Blue") {
	return {
		id: "2.7",
		revision: 5,
		body: {
			name,
			number: 7,
			family: "Color",
			values: { fixture: { color: "#0011ff" } },
			preserved_extension: { mode: "future" },
		},
	};
}

function changedOutcome(requestId = REQUEST_ID) {
	return {
		request_id: requestId,
		correlation_id: CORRELATION_ID,
		replayed: false,
		status: "changed",
		show_revision: 12,
		preset: wirePreset(),
		event_sequence: 42,
	};
}

describe("Preset recording v2 wire", () => {
	it("encodes only the action-time capture request with a snake-case family", () => {
		expect(encodePresetRecordingRequest(request())).toEqual({
			request_id: REQUEST_ID,
			address: { family: "color", number: 7 },
			name: "Deep Blue",
			mode: "merge",
			expected_object_revision: 4,
		});
		expect(JSON.stringify(encodePresetRecordingRequest(request()))).not.toMatch(
			/values|programmer|selection|preload/i,
		);
	});

	it("strictly decodes changed and no-change authoritative outcomes", () => {
		expect(
			decodePresetRecordingOutcome(changedOutcome(), request()),
		).toMatchObject({
			status: "changed",
			requestId: REQUEST_ID,
			eventSequence: 42,
			preset: {
				kind: "preset",
				id: "2.7",
				revision: 5,
				body: { preserved_extension: { mode: "future" } },
			},
		});
		const noChange = changedOutcome();
		noChange.status = "no_change";
		noChange.preset.revision = request().expectedObjectRevision;
		delete (noChange as Partial<typeof noChange>).event_sequence;
		expect(
			decodePresetRecordingOutcome(noChange, request()),
		).toMatchObject({ status: "no_change", showRevision: 12 });
		const legacyIdentity = changedOutcome();
		legacyIdentity.preset.id = "07";
		expect(
			decodePresetRecordingOutcome(legacyIdentity, request()),
		).toMatchObject({ preset: { id: "07" } });
	});

	it("rejects request mismatches, undeclared fields, and invalid event shapes", () => {
		expect(() =>
			decodePresetRecordingOutcome(changedOutcome("other"), request()),
		).toThrow("$.request_id");
		expect(() =>
			decodePresetRecordingOutcome(
				{ ...changedOutcome(), projection: {} },
				request(),
			),
		).toThrow("$.projection");
		const missingSequence = changedOutcome();
		delete (missingSequence as Partial<typeof missingSequence>).event_sequence;
		expect(() =>
			decodePresetRecordingOutcome(missingSequence, request()),
		).toThrow("$.event_sequence");
	});

	it("rejects mismatched Preset identity and unsafe revisions", () => {
		for (const preset of [
			{ ...wirePreset(), body: { ...wirePreset().body, family: "Beam" } },
			{ ...wirePreset(), body: { ...wirePreset().body, number: 8 } },
			{ ...wirePreset(), body: { ...wirePreset().body, name: "Other" } },
			{
				...wirePreset(),
				body: { ...wirePreset().body, group_values: null },
			},
			{ ...wirePreset(), id: "" },
			{ ...wirePreset(), id: "legacy-color" },
			{ ...wirePreset(), id: "3.7" },
			{ ...wirePreset(), id: "bad\u0000id" },
			{ ...wirePreset(), id: "x".repeat(257) },
			{ ...wirePreset(), revision: -1 },
		])
			expect(() =>
				decodePresetRecordingOutcome(
					{ ...changedOutcome(), preset },
					request(),
				),
			).toThrow();
		expect(() =>
			decodePresetRecordingOutcome(
				{ ...changedOutcome(), show_revision: -1 },
				request(),
			),
		).toThrow("$.show_revision");
		expect(() =>
			decodePresetRecordingOutcome(
				{ ...changedOutcome(), event_sequence: -1 },
				request(),
			),
		).toThrow("$.event_sequence");
		expect(() =>
			decodePresetRecordingOutcome(
				{
					...changedOutcome(),
					preset: { ...wirePreset(), revision: 4 },
				},
				request(),
			),
		).toThrow("$.preset.revision");
		const staleNoChange = changedOutcome();
		staleNoChange.status = "no_change";
		delete (staleNoChange as Partial<typeof staleNoChange>).event_sequence;
		expect(() =>
			decodePresetRecordingOutcome(staleNoChange, request()),
		).toThrow("$.preset.revision");
	});

	it("rejects names and request revisions that the backend cannot accept", () => {
		expect(() =>
			encodePresetRecordingRequest({ ...request(), name: "\u0000" }),
		).toThrow("$.name");
		expect(() =>
			encodePresetRecordingRequest({ ...request(), name: "x".repeat(257) }),
		).toThrow("$.name");
		expect(() =>
			encodePresetRecordingRequest({
				...request(),
				expectedObjectRevision: -1,
			}),
		).toThrow("$.expectedObjectRevision");
		expect(() =>
			encodePresetRecordingRequest({
				...request(),
				address: { family: "Color", number: 0 },
			}),
		).toThrow("$.address.number");
	});

	it("strictly decodes conflict metadata", () => {
		expect(
			decodePresetRecordErrorResponse({
				kind: "conflict",
				error: "revision conflict",
				current_revision: 9,
				retryable: false,
			}),
		).toEqual({
			kind: "conflict",
			error: "revision conflict",
			currentRevision: 9,
			retryable: false,
		});
	});
});

describe("Preset recording v2 HTTP adapter", () => {
	it("is dormant until one exact action request is made", async () => {
		const fetchMock = vi.fn(
			async (_input: RequestInfo | URL, _init?: RequestInit) =>
				new Response(JSON.stringify(changedOutcome()), {
					status: 200,
					headers: { "content-type": "application/json" },
				}),
		);
		const transport = new HttpPresetRecordingTransport({
			baseUrl: "http://desk.local/",
			sessionToken: "session-token",
			deskBoundaryToken: "desk-token",
			fetch: fetchMock as typeof fetch,
		});
		expect(fetchMock).not.toHaveBeenCalled();

		await transport.record(SHOW_ID, request());

		expect(fetchMock).toHaveBeenCalledOnce();
		const [url, init] = fetchMock.mock.calls[0];
		expect(String(url)).toBe(
			`http://desk.local/api/v2/shows/${SHOW_ID}/presets/record`,
		);
		expect(init?.method).toBe("POST");
		expect((init?.headers as Headers).get("authorization")).toBe(
			"Bearer session-token",
		);
		expect((init?.headers as Headers).get("x-light-desk-token")).toBe(
			"desk-token",
		);
		expect(String(url)).not.toMatch(/bootstrap|programmers/);
	});

	it("surfaces a typed revision conflict", async () => {
		const transport = new HttpPresetRecordingTransport({
			baseUrl: "http://desk.local",
			sessionToken: "session-token",
			fetch: vi.fn(async () =>
				new Response(
					JSON.stringify({
						kind: "conflict",
						error: "revision conflict",
						current_revision: 7,
						retryable: false,
					}),
					{ status: 409 },
				),
			) as typeof fetch,
		});

		await expect(transport.record(SHOW_ID, request())).rejects.toMatchObject({
			name: "PresetRecordingActionError",
			kind: "conflict",
			status: 409,
			currentRevision: 7,
			retryable: false,
		});
	});
});
