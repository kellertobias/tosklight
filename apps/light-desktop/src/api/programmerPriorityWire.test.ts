import { describe, expect, it } from "vitest";
import type { ProgrammerPriorityActionRequest } from "../features/programmerPriority/contracts";
import {
	CORRELATION_ID,
	OTHER_USER_ID,
	USER_ID,
} from "../features/programmerPriority/testFixtures";
import {
	decodeProgrammerPriorityActionOutcome,
	decodeProgrammerPriorityErrorResponse,
	decodeProgrammerPriorityEventMessage,
	decodeProgrammerPrioritySnapshot,
	encodeProgrammerPriorityActionRequest,
} from "./programmerPriorityWire";
import { WireValidationError } from "./wireValidation";

const REQUEST: ProgrammerPriorityActionRequest = {
	requestId: "priority-request",
	expectedRevision: 4,
	priority: 8,
};

function projection(overrides: Record<string, unknown> = {}) {
	return {
		user_id: USER_ID,
		revision: 5,
		priority: 8,
		changed_at: "2026-07-21T10:00:00.123+02:00",
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
			object: {
				capability: "programmer",
				id: `programming-priority:${USER_ID}`,
			},
			related_objects: [],
			source: { kind: "action", source: "http" },
			correlation_id: CORRELATION_ID,
			delivery: "replaceable",
			payload: { type: "programmer_priority_changed", change },
			...overrides,
		},
	};
}

describe("Programmer priority wire", () => {
	it("encodes only the committed action contract", () => {
		expect(encodeProgrammerPriorityActionRequest(REQUEST)).toEqual({
			request_id: "priority-request",
			expected_revision: 4,
			priority: 8,
		});
		expect(() =>
			encodeProgrammerPriorityActionRequest({ ...REQUEST, priority: 32_768 }),
		).toThrow(WireValidationError);
		expect(() =>
			encodeProgrammerPriorityActionRequest({ ...REQUEST, requestId: "\n" }),
		).toThrow(WireValidationError);
	});

	it("strictly decodes an exact-user snapshot", () => {
		expect(
			decodeProgrammerPrioritySnapshot(
				{ cursor: { sequence: 18 }, projection: projection() },
				USER_ID,
			),
		).toEqual({
			cursor: 18,
			projection: {
				userId: USER_ID,
				revision: 5,
				priority: 8,
				changedAt: "2026-07-21T10:00:00.123+02:00",
			},
		});
	});

	it.each([
		["foreign user", { user_id: OTHER_USER_ID }],
		["negative revision", { revision: -1 }],
		["fractional priority", { priority: 2.5 }],
		["priority overflow", { priority: -32_769 }],
		["impossible timestamp", { changed_at: "2026-02-30T10:00:00Z" }],
		["invalid offset", { changed_at: "2026-07-21T10:00:00+24:00" }],
	])("rejects a snapshot with %s", (_label, replacement) => {
		expect(() =>
			decodeProgrammerPrioritySnapshot(
				{
					cursor: { sequence: 18 },
					projection: projection(replacement),
				},
				USER_ID,
			),
		).toThrow(WireValidationError);
	});

	it("decodes changed, no-change, and replay outcomes", () => {
		expect(
			decodeProgrammerPriorityActionOutcome(
				{
					request_id: REQUEST.requestId,
					correlation_id: CORRELATION_ID,
					projection: projection(),
					status: "changed",
					event_sequence: 19,
					replayed: false,
					warning: null,
				},
				USER_ID,
				REQUEST,
			),
		).toMatchObject({
			status: "changed",
			eventSequence: 19,
			replayed: false,
		});
		expect(
			decodeProgrammerPriorityActionOutcome(
				{
					request_id: REQUEST.requestId,
					correlation_id: CORRELATION_ID,
					projection: projection({ revision: 4 }),
					status: "no_change",
					replayed: true,
				},
				USER_ID,
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
			{ projection: projection({ user_id: OTHER_USER_ID }) },
		],
		["missing correlation", { correlation_id: undefined }],
		["unknown field", { values: [] }],
	])("rejects an outcome with %s", (_label, replacement) => {
		const candidate: Record<string, unknown> = {
			request_id: REQUEST.requestId,
			correlation_id: CORRELATION_ID,
			projection: projection(),
			status: "changed",
			event_sequence: 19,
			replayed: false,
			...replacement,
		};
		if (
			"correlation_id" in replacement &&
			replacement.correlation_id === undefined
		)
			delete candidate.correlation_id;
		expect(() =>
			decodeProgrammerPriorityActionOutcome(candidate, USER_ID, REQUEST),
		).toThrow(WireValidationError);
	});

	it("rejects an event sequence on a no-change outcome", () => {
		expect(() =>
			decodeProgrammerPriorityActionOutcome(
				{
					request_id: REQUEST.requestId,
					correlation_id: CORRELATION_ID,
					projection: projection({ revision: 4 }),
					status: "no_change",
					event_sequence: 19,
					replayed: false,
				},
				USER_ID,
				REQUEST,
			),
		).toThrow(WireValidationError);
	});

	it("decodes exact upsert, remove, and gap messages", () => {
		expect(
			decodeProgrammerPriorityEventMessage(
				event({ type: "upsert", projection: projection() }),
				USER_ID,
			),
		).toMatchObject({
			type: "event",
			sequence: 19,
			change: { type: "upsert", projection: { priority: 8 } },
		});
		expect(
			decodeProgrammerPriorityEventMessage(
				event({ type: "remove", user_id: USER_ID, revision: 6 }),
				USER_ID,
			),
		).toMatchObject({
			change: { type: "remove", userId: USER_ID, revision: 6 },
		});
		expect(
			decodeProgrammerPriorityEventMessage(
				{
					type: "gap",
					gap: {
						after_sequence: 19,
						oldest_available: 25,
						latest_sequence: 30,
					},
				},
				USER_ID,
			),
		).toEqual({
			type: "gap",
			afterSequence: 19,
			oldestAvailable: 25,
			latestSequence: 30,
		});
	});

	it.each([
		["desk-scoped", { desk_id: USER_ID }],
		["lossless", { delivery: "lossless" }],
		[
			"foreign object",
			{
				object: {
					capability: "programmer",
					id: `programming-priority:${OTHER_USER_ID}`,
				},
			},
		],
		[
			"values payload",
			{ payload: { type: "programming_values_changed", change: {} } },
		],
		["missing correlation", { correlation_id: undefined }],
	])("rejects a %s event", (_label, replacement) => {
		const candidate = event(
			{ type: "upsert", projection: projection() },
			replacement,
		);
		if (
			"correlation_id" in replacement &&
			replacement.correlation_id === undefined
		)
			delete (candidate.event as Record<string, unknown>).correlation_id;
		expect(() =>
			decodeProgrammerPriorityEventMessage(candidate, USER_ID),
		).toThrow(WireValidationError);
	});

	it("decodes typed conflicts and rejects undeclared error fields", () => {
		expect(
			decodeProgrammerPriorityErrorResponse({
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
			decodeProgrammerPriorityErrorResponse({
				kind: "conflict",
				error: "revision conflict",
				retryable: false,
				values: [],
			}),
		).toThrow(WireValidationError);
	});
});
