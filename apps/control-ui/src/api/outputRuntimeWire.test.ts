import { describe, expect, it } from "vitest";
import type { OutputRuntimeActionRequest } from "../features/outputRuntime/contracts";
import {
	CORRELATION_ID,
	DESK_ID,
	OTHER_SHOW_ID,
	SHOW_ID,
} from "../features/outputRuntime/testFixtures";
import {
	decodeOutputRuntimeActionOutcome,
	decodeOutputRuntimeErrorResponse,
	decodeOutputRuntimeEventMessage,
	decodeOutputRuntimeSnapshot,
	encodeOutputRuntimeActionRequest,
} from "./outputRuntimeWire";
import { WireValidationError } from "./wireValidation";

const REQUEST: OutputRuntimeActionRequest = {
	requestId: "output-request",
	expectedShowId: SHOW_ID,
	expectedRevision: 4,
	grandMaster: 0.4,
	blackout: true,
};

function projection(overrides: Record<string, unknown> = {}) {
	return {
		scope: { show_id: SHOW_ID },
		identity: "global_master",
		revision: 5,
		grand_master: 0.4,
		blackout: true,
		...overrides,
	};
}

function event(change: unknown, overrides: Record<string, unknown> = {}) {
	return {
		type: "event",
		event: {
			sequence: 19,
			occurred_at: "2026-07-21T10:00:00Z",
			desk_id: null,
			class: "projection",
			object: { capability: "output", id: "runtime:global-master" },
			related_objects: [],
			source: { kind: "action", source: "http" },
			correlation_id: CORRELATION_ID,
			delivery: "replaceable",
			payload: { type: "output_runtime_changed", change },
			...overrides,
		},
	};
}

describe("Output runtime wire", () => {
	it("encodes one sparse or combined committed action", () => {
		expect(encodeOutputRuntimeActionRequest(REQUEST)).toEqual({
			request_id: "output-request",
			expected_show_id: SHOW_ID,
			expected_revision: 4,
			grand_master: 0.4,
			blackout: true,
		});
		expect(
			encodeOutputRuntimeActionRequest({
				...REQUEST,
				grandMaster: undefined,
			}),
		).toEqual({
			request_id: "output-request",
			expected_show_id: SHOW_ID,
			expected_revision: 4,
			blackout: true,
		});
	});

	it.each([
		["empty", { grandMaster: undefined, blackout: undefined }],
		["nonfinite", { grandMaster: Number.NaN }],
		["negative", { grandMaster: -0.1 }],
		["overflow", { grandMaster: 1.1 }],
		["control request ID", { requestId: "bad\nrequest" }],
		["oversized UTF-8 request ID", { requestId: "é".repeat(65) }],
	])("rejects a %s request", (_label, replacement) => {
		expect(() =>
			encodeOutputRuntimeActionRequest({ ...REQUEST, ...replacement }),
		).toThrow(WireValidationError);
	});

	it("strictly decodes an exact-Show snapshot", () => {
		expect(
			decodeOutputRuntimeSnapshot(
				{ cursor: { sequence: 18 }, projection: projection() },
				SHOW_ID,
			),
		).toEqual({
			cursor: 18,
			projection: {
				showId: SHOW_ID,
				identity: "global_master",
				revision: 5,
				grandMaster: 0.4,
				blackout: true,
			},
		});
	});

	it.each([
		["foreign Show", { scope: { show_id: OTHER_SHOW_ID } }],
		["missing Show", { scope: {} }],
		["negative revision", { revision: -1 }],
		["nonfinite level", { grand_master: Number.POSITIVE_INFINITY }],
		["out-of-range level", { grand_master: 2 }],
		["nonboolean blackout", { blackout: 0 }],
		["unknown field", { values: [] }],
	])("rejects a snapshot with %s", (_label, replacement) => {
		expect(() =>
			decodeOutputRuntimeSnapshot(
				{
					cursor: { sequence: 18 },
					projection: projection(replacement),
				},
				SHOW_ID,
			),
		).toThrow(WireValidationError);
	});

	it("decodes changed, no-change, replay, durability, and warning", () => {
		expect(
			decodeOutputRuntimeActionOutcome(
				{
					request_id: REQUEST.requestId,
					correlation_id: CORRELATION_ID,
					projection: projection(),
					status: "changed",
					event_sequence: 19,
					replayed: false,
					durability: "persistence_pending",
					warning: "persistence pending",
				},
				SHOW_ID,
				REQUEST,
			),
		).toMatchObject({
			status: "changed",
			eventSequence: 19,
			replayed: false,
			durability: "persistence_pending",
			warning: "persistence pending",
		});
		expect(
			decodeOutputRuntimeActionOutcome(
				{
					request_id: REQUEST.requestId,
					correlation_id: CORRELATION_ID,
					projection: projection({ revision: 4 }),
					status: "no_change",
					replayed: true,
					durability: "durable",
				},
				SHOW_ID,
				REQUEST,
			),
		).toMatchObject({
			status: "no_change",
			eventSequence: null,
			replayed: true,
			warning: null,
		});
	});

	it.each([
		["wrong request", { request_id: "another-request" }],
		["wrong revision", { projection: projection({ revision: 8 }) }],
		[
			"foreign response",
			{ projection: projection({ scope: { show_id: OTHER_SHOW_ID } }) },
		],
		["missing correlation", { correlation_id: undefined }],
		["missing durability", { durability: undefined }],
		["unknown field", { values: [] }],
	])("rejects an outcome with %s", (_label, replacement) => {
		const candidate: Record<string, unknown> = {
			request_id: REQUEST.requestId,
			correlation_id: CORRELATION_ID,
			projection: projection(),
			status: "changed",
			event_sequence: 19,
			replayed: false,
			durability: "durable",
			...replacement,
		};
		for (const [key, value] of Object.entries(replacement))
			if (value === undefined) delete candidate[key];
		expect(() =>
			decodeOutputRuntimeActionOutcome(candidate, SHOW_ID, REQUEST),
		).toThrow(WireValidationError);
	});

	it("rejects event sequence materialization on a no-change outcome", () => {
		expect(() =>
			decodeOutputRuntimeActionOutcome(
				{
					request_id: REQUEST.requestId,
					correlation_id: CORRELATION_ID,
					projection: projection({ revision: 4 }),
					status: "no_change",
					event_sequence: 19,
					replayed: false,
					durability: "durable",
				},
				SHOW_ID,
				REQUEST,
			),
		).toThrow(WireValidationError);
	});

	it("decodes exact change and gap messages", () => {
		expect(
			decodeOutputRuntimeEventMessage(
				event({ projection: projection() }),
				SHOW_ID,
			),
		).toMatchObject({
			type: "event",
			sequence: 19,
			change: { projection: { grandMaster: 0.4, blackout: true } },
		});
		expect(
			decodeOutputRuntimeEventMessage(
				{
					type: "gap",
					gap: {
						after_sequence: 19,
						oldest_available: 25,
						latest_sequence: 30,
					},
				},
				SHOW_ID,
			),
		).toEqual({
			type: "gap",
			afterSequence: 19,
			oldestAvailable: 25,
			latestSequence: 30,
		});
	});

	it.each([
		["desk-scoped", { desk_id: DESK_ID }],
		["lossless", { delivery: "lossless" }],
		["runtime source", { source: { kind: "runtime" } }],
		["impossible timestamp", { occurred_at: "2026-02-30T10:00:00Z" }],
		["invalid offset", { occurred_at: "2026-07-21T10:00:00+24:00" }],
		[
			"foreign object",
			{ object: { capability: "playback", id: "runtime:global-master" } },
		],
		[
			"wrong payload",
			{ payload: { type: "playback_runtime_changed", change: {} } },
		],
		["missing correlation", { correlation_id: undefined }],
	])("rejects a %s event", (_label, replacement) => {
		const candidate = event({ projection: projection() }, replacement);
		if (
			"correlation_id" in replacement &&
			replacement.correlation_id === undefined
		)
			delete (candidate.event as Record<string, unknown>).correlation_id;
		expect(() => decodeOutputRuntimeEventMessage(candidate, SHOW_ID)).toThrow(
			WireValidationError,
		);
	});

	it("decodes typed conflicts and rejects undeclared error fields", () => {
		expect(
			decodeOutputRuntimeErrorResponse({
				kind: "conflict",
				error: "revision conflict",
				current_revision: 7,
				retryable: false,
			}),
		).toEqual({
			kind: "conflict",
			error: "revision conflict",
			currentRevision: 7,
			retryable: false,
		});
		expect(() =>
			decodeOutputRuntimeErrorResponse({
				kind: "conflict",
				error: "revision conflict",
				retryable: false,
				projection: {},
			}),
		).toThrow(WireValidationError);
	});
});
