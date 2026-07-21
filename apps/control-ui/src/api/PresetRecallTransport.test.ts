import { describe, expect, it, vi } from "vitest";
import type { PresetRecallRequest } from "../features/presetRecall/contracts";
import { HttpPresetRecallTransport } from "./PresetRecallTransport";
import {
	decodePresetRecallErrorResponse,
	decodePresetRecallOutcome,
	encodePresetRecallRequest,
} from "./presetRecallWire";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const USER_ID = "22222222-2222-4222-8222-222222222222";
const DESK_ID = "33333333-3333-4333-8333-333333333333";
const REQUEST_ID = "44444444-4444-4444-8444-444444444444";
const CORRELATION_ID = "55555555-5555-4555-8555-555555555555";
const FIXTURE_ID = "66666666-6666-4666-8666-666666666666";

function request(
	overrides: Partial<PresetRecallRequest> = {},
): PresetRecallRequest {
	return {
		requestId: REQUEST_ID,
		presetId: "2.7",
		address: { family: "Color", number: 7 },
		expectedPresetRevision: 4,
		expectedShowRevision: 12,
		expectedProgrammerRevision: 6,
		expectedCaptureModeRevision: 3,
		expectedSelectionRevision: 8,
		selectedFixtureCount: 1,
		...overrides,
	};
}

function projection(revision = 7) {
	return {
		user_id: USER_ID,
		revision,
		fixture_values: [
			{
				fixture_id: FIXTURE_ID,
				attribute: "color",
				value: { kind: "color_xyz", value: { x: 0.2, y: 0.3, z: 0.4 } },
				programmer_order: 1,
				fade: true,
				fade_millis: 500,
				delay_millis: null,
			},
		],
		group_values: [],
	};
}

function preset() {
	return {
		id: "2.7",
		revision: 4,
		body: {
			name: "Deep Blue",
			number: 7,
			family: "Color",
			values: { [FIXTURE_ID]: { color: { kind: "normalized", value: 1 } } },
			group_values: {},
			future_extension: { retained: true },
		},
	};
}

function changedOutcome() {
	return {
		request_id: REQUEST_ID,
		correlation_id: CORRELATION_ID,
		replayed: false,
		show_revision: 12,
		programmer_revision: 7,
		capture_mode_revision: 3,
		selection_revision: 8,
		applied_fixtures: 1,
		active_context: "preset:2.7",
		preset: preset(),
		status: "changed",
		projection: projection(),
		event_sequence: 41,
	};
}

describe("Preset recall v2 wire", () => {
	it("encodes only captured revisions and the operator address", () => {
		expect(encodePresetRecallRequest(request())).toEqual({
			request_id: REQUEST_ID,
			address: { family: "color", number: 7 },
			expected_preset_revision: 4,
			expected_show_revision: 12,
			expected_programmer_revision: 6,
			expected_capture_mode_revision: 3,
			expected_selection_revision: 8,
		});
		expect(JSON.stringify(encodePresetRecallRequest(request()))).not.toMatch(
			/presetId|selectedFixtureCount|fixture_values|group_values/,
		);
	});

	it("decodes a lossless values change and replay", () => {
		const decoded = decodePresetRecallOutcome(
			changedOutcome(),
			USER_ID,
			request(),
		);
		expect(decoded).toMatchObject({
			status: "changed",
			programmerRevision: 7,
			eventSequence: 41,
			projection: { userId: USER_ID, revision: 7 },
			preset: {
				id: "2.7",
				revision: 4,
				body: { future_extension: { retained: true } },
			},
		});
		const replay = { ...changedOutcome(), replayed: true };
		expect(decodePresetRecallOutcome(replay, USER_ID, request())).toMatchObject(
			{ replayed: true, eventSequence: 41 },
		);
	});

	it("accepts interaction-only, context-only, and no-change sparse outcomes", () => {
		const sparse = changedOutcome();
		delete (sparse as Partial<typeof sparse>).projection;
		delete (sparse as Partial<typeof sparse>).event_sequence;
		sparse.programmer_revision = 6;
		sparse.selection_revision = 9;
		(
			sparse as typeof sparse & { interaction_event_sequence: number }
		).interaction_event_sequence = 42;
		expect(decodePresetRecallOutcome(sparse, USER_ID, request())).toMatchObject(
			{
				status: "changed",
				projection: null,
				eventSequence: null,
				selectionRevision: 9,
				interactionEventSequence: 42,
			},
		);

		delete (sparse as { interaction_event_sequence?: number })
			.interaction_event_sequence;
		sparse.selection_revision = 8;
		expect(decodePresetRecallOutcome(sparse, USER_ID, request())).toMatchObject(
			{ status: "changed", interactionEventSequence: null },
		);

		sparse.status = "no_change";
		expect(decodePresetRecallOutcome(sparse, USER_ID, request())).toMatchObject(
			{ status: "no_change", projection: null },
		);
	});

	it("rejects mismatched authority and malformed sparse values", () => {
		for (const candidate of [
			{ ...changedOutcome(), request_id: "another" },
			{ ...changedOutcome(), show_revision: 13 },
			{ ...changedOutcome(), capture_mode_revision: 4 },
			{ ...changedOutcome(), applied_fixtures: 2 },
			{ ...changedOutcome(), active_context: "preset:2.8" },
			{ ...changedOutcome(), preset: { ...preset(), id: "legacy" } },
			{ ...changedOutcome(), preset: { ...preset(), revision: 5 } },
			{
				...changedOutcome(),
				preset: {
					...preset(),
					body: { ...preset().body, family: "Beam" },
				},
			},
			{ ...changedOutcome(), programmer_revision: 8 },
			{ ...changedOutcome(), projection: projection(8) },
			{ ...changedOutcome(), extra: true },
		])
			expect(() =>
				decodePresetRecallOutcome(candidate, USER_ID, request()),
			).toThrow();

		const missingEvent = changedOutcome();
		delete (missingEvent as Partial<typeof missingEvent>).event_sequence;
		expect(() =>
			decodePresetRecallOutcome(missingEvent, USER_ID, request()),
		).toThrow(/paired values/);
		const materializedNoChange = { ...changedOutcome(), status: "no_change" };
		expect(() =>
			decodePresetRecallOutcome(materializedNoChange, USER_ID, request()),
		).toThrow(/no_change/);
	});

	it("strictly decodes revision conflict metadata", () => {
		expect(
			decodePresetRecallErrorResponse({
				kind: "conflict",
				error: "selection revision conflict",
				current_revision: 7,
				current_related_revision: 9,
				retryable: false,
			}),
		).toEqual({
			kind: "conflict",
			error: "selection revision conflict",
			currentRevision: 7,
			currentRelatedRevision: 9,
			retryable: false,
		});
		expect(() =>
			decodePresetRecallErrorResponse({
				kind: "conflict",
				error: "conflict",
				retryable: false,
				extra: true,
			}),
		).toThrow(/declared wire field/);
	});
});

describe("Preset recall v2 HTTP adapter", () => {
	it("is dormant until one exact action and sends authenticated v2 HTTP", async () => {
		const fetchMock = vi.fn(
			async (_input: RequestInfo | URL, _init?: RequestInit) =>
				new Response(JSON.stringify(changedOutcome()), { status: 200 }),
		);
		const transport = new HttpPresetRecallTransport({
			baseUrl: "http://desk.local/",
			sessionToken: "session-token",
			deskBoundaryToken: "desk-token",
			fetch: fetchMock as typeof fetch,
		});
		expect(fetchMock).not.toHaveBeenCalled();

		await transport.recall(
			{ showId: SHOW_ID, userId: USER_ID, deskId: DESK_ID },
			request(),
		);

		expect(fetchMock).toHaveBeenCalledOnce();
		const [url, init] = fetchMock.mock.calls[0];
		expect(String(url)).toBe(
			`http://desk.local/api/v2/shows/${SHOW_ID}/presets/recall`,
		);
		expect(init?.method).toBe("POST");
		expect((init?.headers as Headers).get("authorization")).toBe(
			"Bearer session-token",
		);
		expect((init?.headers as Headers).get("x-light-desk-token")).toBe(
			"desk-token",
		);
		expect(String(url)).not.toMatch(/bootstrap|playbacks|programmers/);
	});

	it("surfaces typed conflicts and retryable network failures", async () => {
		const conflict = new HttpPresetRecallTransport({
			baseUrl: "http://desk.local",
			sessionToken: "token",
			fetch: vi.fn(
				async () =>
					new Response(
						JSON.stringify({
							kind: "conflict",
							error: "revision conflict",
							current_revision: 7,
							current_related_revision: 9,
							retryable: false,
						}),
						{ status: 409 },
					),
			) as typeof fetch,
		});
		await expect(
			conflict.recall(
				{ showId: SHOW_ID, userId: USER_ID, deskId: DESK_ID },
				request(),
			),
		).rejects.toMatchObject({
			name: "PresetRecallTransportError",
			status: 409,
			currentRevision: 7,
			currentRelatedRevision: 9,
			retryable: false,
		});

		const unavailable = new HttpPresetRecallTransport({
			baseUrl: "http://desk.local",
			sessionToken: "token",
			fetch: vi.fn(async () => {
				throw new TypeError("offline");
			}) as typeof fetch,
		});
		await expect(
			unavailable.recall(
				{ showId: SHOW_ID, userId: USER_ID, deskId: DESK_ID },
				request(),
			),
		).rejects.toMatchObject({ status: 0, retryable: true });
	});
});
