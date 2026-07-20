import { describe, expect, it } from "vitest";
import {
	decodeProgrammerPreloadValuesActionOutcome,
	decodeProgrammerPreloadValuesErrorResponse,
	decodeProgrammerPreloadValuesEventMessage,
	decodeProgrammerPreloadValuesSnapshot,
	encodeProgrammerPreloadValuesActionRequest,
} from "./programmerPreloadValuesWire";
import { WireValidationError } from "./wireValidation";

const USER_ID = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const OTHER_USER_ID = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
const FIXTURE_ID = "11111111-1111-4111-8111-111111111111";
const CORRELATION_ID = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";

function projection(userId = USER_ID, revision = 7) {
	return {
		user_id: userId,
		revision,
		fixture_values: [
			{
				fixture_id: FIXTURE_ID,
				attribute: "intensity",
				value: { kind: "normalized", value: 0.75 },
				programmer_order: 9,
				fade: true,
				fade_millis: 1_000,
				delay_millis: 250,
			},
		],
		group_values: [
			{
				group_id: "front",
				attribute: "color",
				value: {
					kind: "color_xyz",
					value: { x: 0.1, y: 0.2, z: 0.3 },
				},
				programmer_order: 10,
				fade: false,
			},
		],
	};
}

function snapshot() {
	return { cursor: { sequence: 18 }, projection: projection() };
}

function changedOutcome(userId = USER_ID) {
	return {
		request_id: "request-1",
		correlation_id: CORRELATION_ID,
		revision: 7,
		capture_mode_revision: 4,
		status: "changed",
		projection: projection(userId),
		event_sequence: 19,
		replayed: false,
		warning: null,
	};
}

function preloadEvent(userId = USER_ID) {
	return {
		type: "event",
		event: {
			sequence: 19,
			occurred_at: "2026-07-20T12:00:00Z",
			desk_id: null,
			class: "projection",
			object: {
				capability: "programmer",
				id: `programming-preload-values:${userId}`,
			},
			related_objects: [],
			source: { kind: "action", source: "http" },
			correlation_id: CORRELATION_ID,
			delivery: "replaceable",
			payload: {
				type: "programming_preload_values_changed",
				change: { projection: projection(userId) },
			},
		},
	};
}

function record(value: unknown) {
	return value as Record<string, unknown>;
}

function addExtra(value: unknown) {
	record(value).extra = true;
}

describe("Preload Programmer values snapshot wire", () => {
	it("decodes only pending exact-user values with timing and order", () => {
		expect(decodeProgrammerPreloadValuesSnapshot(snapshot(), USER_ID)).toEqual({
			cursor: 18,
			projection: {
				userId: USER_ID,
				revision: 7,
				fixtureValues: [
					{
						fixtureId: FIXTURE_ID,
						attribute: "intensity",
						value: { kind: "normalized", value: 0.75 },
						programmerOrder: 9,
						fade: true,
						fadeMillis: 1_000,
						delayMillis: 250,
					},
				],
				groupValues: [
					expect.objectContaining({
						groupId: "front",
						programmerOrder: 10,
						fadeMillis: null,
						delayMillis: null,
					}),
				],
			},
		});
	});

	it("rejects a foreign-user snapshot", () => {
		const candidate = snapshot();
		candidate.projection.user_id = OTHER_USER_ID;
		expect(() =>
			decodeProgrammerPreloadValuesSnapshot(candidate, USER_ID),
		).toThrow(/requested user/);
	});

	it("rejects unknown fields at every snapshot object level", () => {
		const mutations: Array<(candidate: ReturnType<typeof snapshot>) => void> = [
			(candidate) => addExtra(candidate),
			(candidate) => addExtra(candidate.cursor),
			(candidate) => addExtra(candidate.projection),
			(candidate) => addExtra(candidate.projection.fixture_values[0]),
			(candidate) => addExtra(candidate.projection.fixture_values[0]?.value),
			(candidate) => addExtra(candidate.projection.group_values[0]),
			(candidate) => addExtra(candidate.projection.group_values[0]?.value),
			(candidate) =>
				addExtra(record(candidate.projection.group_values[0]?.value).value),
		];

		for (const mutate of mutations) {
			const candidate = structuredClone(snapshot());
			mutate(candidate);
			expect(() =>
				decodeProgrammerPreloadValuesSnapshot(candidate, USER_ID),
			).toThrow(/declared wire field/);
		}
	});
});

describe("Preload Programmer values mutation wire", () => {
	it("maps Preload revisions and preserves one ordered batch", () => {
		expect(
			encodeProgrammerPreloadValuesActionRequest({
				requestId: "batch-1",
				expectedPreloadRevision: 6,
				expectedCaptureModeRevision: 4,
				action: {
					action: "batch",
					mutations: [
						{
							action: "set_fixture",
							fixtureId: FIXTURE_ID,
							attribute: "intensity",
							value: { kind: "normalized", value: 0.5 },
							timing: {
								fade: true,
								fadeMillis: 500,
								delayMillis: null,
							},
						},
						{
							action: "release_group",
							groupId: "front",
							attribute: "intensity",
						},
					],
				},
			}),
		).toEqual({
			request_id: "batch-1",
			expected_revision: 6,
			expected_capture_mode_revision: 4,
			action: {
				type: "batch",
				mutations: [
					{
						type: "set_fixture",
						fixture_id: FIXTURE_ID,
						attribute: "intensity",
						value: { kind: "normalized", value: 0.5 },
						timing: {
							fade: true,
							fade_millis: 500,
							delay_millis: null,
						},
					},
					{
						type: "release_group",
						group_id: "front",
						attribute: "intensity",
					},
				],
			},
		});
	});

	it("decodes changed and sparse no-change outcomes", () => {
		expect(
			decodeProgrammerPreloadValuesActionOutcome(
				changedOutcome(),
				USER_ID,
				"request-1",
			),
		).toMatchObject({
			status: "changed",
			requestId: "request-1",
			preloadRevision: 7,
			captureModeRevision: 4,
			eventSequence: 19,
			projection: { userId: USER_ID, revision: 7 },
		});
		const noChange = changedOutcome();
		noChange.status = "no_change";
		delete (noChange as Partial<ReturnType<typeof changedOutcome>>).projection;
		delete (noChange as Partial<ReturnType<typeof changedOutcome>>)
			.event_sequence;
		expect(
			decodeProgrammerPreloadValuesActionOutcome(
				noChange,
				USER_ID,
				"request-1",
			),
		).toEqual({
			status: "no_change",
			requestId: "request-1",
			correlationId: CORRELATION_ID,
			preloadRevision: 7,
			captureModeRevision: 4,
			replayed: false,
			warning: null,
		});
	});

	it("rejects a foreign action projection, materialized no-op, and extras", () => {
		expect(() =>
			decodeProgrammerPreloadValuesActionOutcome(
				changedOutcome(OTHER_USER_ID),
				USER_ID,
				"request-1",
			),
		).toThrow(/requested user/);
		const noOp = changedOutcome();
		noOp.status = "no_change";
		expect(() =>
			decodeProgrammerPreloadValuesActionOutcome(noOp, USER_ID, "request-1"),
		).toThrow(/no projection/);
		expect(() =>
			decodeProgrammerPreloadValuesActionOutcome(
				{ ...changedOutcome(), extra: true },
				USER_ID,
				"request-1",
			),
		).toThrow(/declared wire field/);
	});

	it("strictly decodes typed revision conflicts", () => {
		expect(
			decodeProgrammerPreloadValuesErrorResponse({
				kind: "conflict",
				error: "revision conflict",
				current_revision: 8,
				current_capture_mode_revision: 5,
				retryable: false,
			}),
		).toEqual({
			kind: "conflict",
			error: "revision conflict",
			currentPreloadRevision: 8,
			currentCaptureModeRevision: 5,
			retryable: false,
		});
		expect(() =>
			decodeProgrammerPreloadValuesErrorResponse({
				kind: "conflict",
				error: "revision conflict",
				retryable: false,
				extra: true,
			}),
		).toThrow(/declared wire field/);
	});
});

describe("Preload Programmer values event wire", () => {
	it("decodes only the exact replaceable user projection object", () => {
		expect(
			decodeProgrammerPreloadValuesEventMessage(preloadEvent(), USER_ID),
		).toEqual({
			type: "event",
			sequence: 19,
			correlationId: CORRELATION_ID,
			projection: expect.objectContaining({ userId: USER_ID, revision: 7 }),
		});
	});

	it("rejects foreign object and projection users", () => {
		const foreignObject = preloadEvent();
		foreignObject.event.object.id = `programming-preload-values:${OTHER_USER_ID}`;
		expect(() =>
			decodeProgrammerPreloadValuesEventMessage(foreignObject, USER_ID),
		).toThrow(WireValidationError);
		const foreignProjection = preloadEvent();
		foreignProjection.event.payload.change.projection.user_id = OTHER_USER_ID;
		expect(() =>
			decodeProgrammerPreloadValuesEventMessage(foreignProjection, USER_ID),
		).toThrow(WireValidationError);
	});

	it("rejects unknown fields at every event envelope object level", () => {
		const mutations: Array<
			(candidate: ReturnType<typeof preloadEvent>) => void
		> = [
			(candidate) => addExtra(candidate),
			(candidate) => addExtra(candidate.event),
			(candidate) => addExtra(candidate.event.object),
			(candidate) => addExtra(candidate.event.source),
			(candidate) => addExtra(candidate.event.payload),
			(candidate) => addExtra(candidate.event.payload.change),
		];

		for (const mutate of mutations) {
			const candidate = structuredClone(preloadEvent());
			mutate(candidate);
			expect(() =>
				decodeProgrammerPreloadValuesEventMessage(candidate, USER_ID),
			).toThrow(/declared wire field/);
		}
	});

	it("strictly decodes cursor and gap control messages", () => {
		expect(
			decodeProgrammerPreloadValuesEventMessage(
				{ type: "ready", cursor: { sequence: 3 } },
				USER_ID,
			),
		).toEqual({ type: "ready", cursor: 3 });
		expect(() =>
			decodeProgrammerPreloadValuesEventMessage(
				{ type: "ready", cursor: { sequence: 3, extra: true } },
				USER_ID,
			),
		).toThrow(/declared wire field/);
		expect(() =>
			decodeProgrammerPreloadValuesEventMessage(
				{
					type: "gap",
					gap: {
						after_sequence: 3,
						oldest_available: 5,
						latest_sequence: 9,
						extra: true,
					},
				},
				USER_ID,
			),
		).toThrow(/declared wire field/);
	});
});
