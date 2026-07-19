import { describe, expect, it } from "vitest";
import { WireValidationError } from "./wireValidation";
import {
	decodeProgrammerValuesActionOutcome,
	decodeProgrammerValuesErrorResponse,
	decodeProgrammerValuesEventMessage,
	decodeProgrammerValuesSnapshot,
	encodeProgrammerValuesActionRequest,
} from "./programmerValuesWire";

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
		group_values: [],
	};
}

function changedOutcome() {
	return {
		request_id: "request-1",
		correlation_id: CORRELATION_ID,
		revision: 7,
		status: "changed",
		projection: projection(),
		event_sequence: 19,
		replayed: false,
		warning: null,
	};
}

function valuesEvent() {
	return {
		type: "event",
		event: {
			sequence: 19,
			occurred_at: "2026-07-19T12:00:00Z",
			desk_id: null,
			class: "projection",
			object: {
				capability: "programmer",
				id: `programming-values:${USER_ID}`,
			},
			related_objects: [],
			source: { kind: "action", source: "http" },
			correlation_id: CORRELATION_ID,
			delivery: "replaceable",
			payload: {
				type: "programming_values_changed",
				change: { projection: projection() },
			},
		},
	};
}

describe("Programmer values wire projection", () => {
	it("decodes an exact-user snapshot without legacy bootstrap fields", () => {
		expect(
			decodeProgrammerValuesSnapshot(
				{ cursor: { sequence: 18 }, projection: projection() },
				USER_ID,
			),
		).toEqual({
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
				groupValues: [],
			},
		});
	});

	it.each([
		{ kind: "spread", value: [0, 1] },
		{ kind: "discrete", value: "open" },
		{ kind: "color_xyz", value: { x: 0.1, y: 0.2, z: 0.3 } },
		{ kind: "raw_dmx", value: 255 },
		{ kind: "raw_dmx_exact", value: 65_535 },
	])("decodes a valid $kind attribute value", (value) => {
		const candidate = projection();
		(candidate.fixture_values[0] as { value: unknown }).value = value;
		expect(
			decodeProgrammerValuesSnapshot(
				{ cursor: { sequence: 1 }, projection: candidate },
				USER_ID,
			).projection.fixtureValues[0].value,
		).toEqual(value);
	});

	it("rejects foreign users and invalid attribute values", () => {
		expect(() =>
			decodeProgrammerValuesSnapshot(
				{ cursor: { sequence: 1 }, projection: projection(OTHER_USER_ID) },
				USER_ID,
			),
		).toThrow(/requested user/);
		const candidate = projection();
		(candidate.fixture_values[0] as { value: unknown }).value = {
			kind: "normalized",
			value: 1.1,
		};
		expect(() =>
			decodeProgrammerValuesSnapshot(
				{ cursor: { sequence: 1 }, projection: candidate },
				USER_ID,
			),
		).toThrow(WireValidationError);
	});
});

describe("Programmer values mutation wire boundary", () => {
	it("decodes changed and no-change outcomes with different payload shapes", () => {
		expect(
			decodeProgrammerValuesActionOutcome(
				changedOutcome(),
				USER_ID,
				"request-1",
			),
		).toMatchObject({
			status: "changed",
			requestId: "request-1",
			revision: 7,
			eventSequence: 19,
			projection: { userId: USER_ID, revision: 7 },
		});
		const noChange = {
			...changedOutcome(),
			status: "no_change",
		};
		delete (noChange as Partial<ReturnType<typeof changedOutcome>>).projection;
		delete (noChange as Partial<ReturnType<typeof changedOutcome>>)
			.event_sequence;
		expect(
			decodeProgrammerValuesActionOutcome(noChange, USER_ID, "request-1"),
		).toEqual({
			status: "no_change",
			requestId: "request-1",
			correlationId: CORRELATION_ID,
			revision: 7,
			replayed: false,
			warning: null,
		});
	});

	it("rejects mismatched requests, foreign projections, and materialized no-ops", () => {
		expect(() =>
			decodeProgrammerValuesActionOutcome(
				changedOutcome(),
				USER_ID,
				"another-request",
			),
		).toThrow(/another-request/);
		const foreign = changedOutcome();
		foreign.projection = projection(OTHER_USER_ID);
		expect(() =>
			decodeProgrammerValuesActionOutcome(foreign, USER_ID, "request-1"),
		).toThrow(/requested user/);
		const materializedNoOp = changedOutcome();
		materializedNoOp.status = "no_change";
		expect(() =>
			decodeProgrammerValuesActionOutcome(
				materializedNoOp,
				USER_ID,
				"request-1",
			),
		).toThrow(/no projection/);
	});

	it("encodes a batch as one action with nested timing", () => {
		expect(
			encodeProgrammerValuesActionRequest({
				requestId: "batch-1",
				expectedRevision: 6,
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

	it("decodes typed revision conflicts", () => {
		expect(
			decodeProgrammerValuesErrorResponse({
				kind: "conflict",
				error: "revision conflict",
				current_revision: 8,
				retryable: false,
			}),
		).toEqual({
			kind: "conflict",
			error: "revision conflict",
			currentRevision: 8,
			retryable: false,
		});
	});
});

describe("Programmer values event wire boundary", () => {
	it("decodes only the exact replaceable user projection", () => {
		expect(decodeProgrammerValuesEventMessage(valuesEvent(), USER_ID)).toEqual({
			type: "event",
			sequence: 19,
			correlationId: CORRELATION_ID,
			projection: expect.objectContaining({ userId: USER_ID, revision: 7 }),
		});
	});

	it.each([
		[
			"foreign route",
			(event: ReturnType<typeof valuesEvent>) => {
				event.event.object.id = `programming-values:${OTHER_USER_ID}`;
			},
		],
		[
			"foreign projection",
			(event: ReturnType<typeof valuesEvent>) => {
				event.event.payload.change.projection.user_id = OTHER_USER_ID;
			},
		],
		[
			"desk-owned envelope",
			(event: ReturnType<typeof valuesEvent>) => {
				(event.event as { desk_id: string | null }).desk_id = FIXTURE_ID;
			},
		],
		[
			"lossless delivery",
			(event: ReturnType<typeof valuesEvent>) => {
				event.event.delivery = "lossless";
			},
		],
	])("rejects a %s", (_label, mutate) => {
		const event = valuesEvent();
		mutate(event);
		expect(() => decodeProgrammerValuesEventMessage(event, USER_ID)).toThrow(
			WireValidationError,
		);
	});
});
