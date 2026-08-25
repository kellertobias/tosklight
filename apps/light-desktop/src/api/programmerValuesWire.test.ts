import { describe, expect, it } from "vitest";
import {
	decodeProgrammerValuesActionOutcome,
	decodeProgrammerValuesErrorResponse,
	decodeProgrammerValuesEventMessage,
	decodeProgrammerValuesSnapshot,
	encodeProgrammerValuesActionRequest,
} from "./programmerValuesWire";
import { WireValidationError } from "./wireValidation";

const SESSION_ID = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const OTHER_SESSION_ID = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
const FIXTURE_ID = "11111111-1111-4111-8111-111111111111";
const CORRELATION_ID = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";

function projection(revision = 7) {
	return {
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
		dynamic_values: [],
	};
}

function changedOutcome() {
	return {
		request_id: "request-1",
		correlation_id: CORRELATION_ID,
		revision: 7,
		capture_mode_revision: 4,
		status: "changed",
		projection: projection(),
		event_sequence: 19,
		replayed: false,
		warning: null,
	};
}

function valuesEvent() {
	const changed = projection();
	return {
		type: "event",
		event: {
			sequence: 19,
			occurred_at: "2026-07-19T12:00:00Z",
			desk_id: null,
			class: "projection",
			object: {
				capability: "programmer",
				id: "programming-values",
			},
			related_objects: [],
			source: { kind: "action", source: "http" },
			correlation_id: CORRELATION_ID,
			delivery: "lossless",
			payload: {
				type: "programming_values_changed",
				change: {
					...changed,
					removed_fixture_values: [],
					removed_group_values: [],
					removed_dynamic_values: [],
				},
			},
		},
	};
}

describe("Programmer values wire projection", () => {
	it("decodes a snapshot without legacy bootstrap fields", () => {
		expect(
			decodeProgrammerValuesSnapshot(
				{ cursor: { sequence: 18 }, projection: projection() }),
		).toEqual({
			cursor: 18,
			projection: {
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
				{ cursor: { sequence: 1 }, projection: candidate }).projection.fixtureValues[0].value,
		).toEqual(value);
	});

	it("decodes retained Dynamic semantic values and rejects undeclared fields", () => {
		const candidate = projection();
		const dynamicValues = candidate.dynamic_values as unknown[];
		dynamicValues.push({
			fixture_id: FIXTURE_ID,
			attribute: "intensity",
			value: { type: "release" },
			programmer_order: 10,
			changed_at_millis: 1_234,
		});
		expect(
			decodeProgrammerValuesSnapshot(
				{ cursor: { sequence: 1 }, projection: candidate }).projection.dynamicValues,
		).toEqual([
			{
				fixtureId: FIXTURE_ID,
				attribute: "intensity",
				value: { type: "release" },
				programmerOrder: 10,
				changedAtMillis: 1_234,
			},
		]);
		(dynamicValues[0] as { value: { extra?: boolean } }).value.extra = true;
		expect(() =>
			decodeProgrammerValuesSnapshot(
				{ cursor: { sequence: 1 }, projection: candidate }),
		).toThrow(/declared wire field/);
	});

	it("accepts the generated spatial mapping field on embedded Dynamics", () => {
		const candidate = projection() as ReturnType<typeof projection> & {
			dynamic_definitions: unknown[];
		};
		candidate.dynamic_definitions = [
			{
				id: "dddddddd-dddd-4ddd-8ddd-dddddddddddd",
				pool_number: 1,
				revision: 2,
				name: "Spatial chase",
				color: null,
				icon: null,
				target_binding: null,
				lanes: [],
				random_groups: [],
				phase_mode: "absolute",
				spatial_mapping: {
					source: { type: "stage" },
					mapping: { type: "radial", center_x: 0.5, center_y: 0.5 },
				},
				phase: 0,
				speed: 1,
				overall_speed_multiplier: 1,
				run_mode: "loop",
				default_activation: null,
				activation_boundary: null,
			},
		];

		expect(
			decodeProgrammerValuesSnapshot(
				{ cursor: { sequence: 1 }, projection: candidate }),
		).toMatchObject({ projection: { revision: 7 } });
	});

	it("rejects an undeclared field and invalid attribute values", () => {
		expect(() =>
			decodeProgrammerValuesSnapshot({
				cursor: { sequence: 1 },
				projection: { ...projection(), user_id: OTHER_SESSION_ID },
			}),
		).toThrow(/user_id/);
		const candidate = projection();
		(candidate.fixture_values[0] as { value: unknown }).value = {
			kind: "normalized",
			value: 1.1,
		};
		expect(() =>
			decodeProgrammerValuesSnapshot(
				{ cursor: { sequence: 1 }, projection: candidate }),
		).toThrow(WireValidationError);
	});

	it("rejects undeclared snapshot and projection fields", () => {
		const snapshot = {
			cursor: { sequence: 1 },
			projection: projection(),
			extra: true,
		};
		expect(() => decodeProgrammerValuesSnapshot(snapshot)).toThrow(
			/declared wire field/,
		);
		delete (snapshot as { extra?: boolean }).extra;
		(
			snapshot.projection as ReturnType<typeof projection> & { extra?: boolean }
		).extra = true;
		expect(() => decodeProgrammerValuesSnapshot(snapshot)).toThrow(
			/declared wire field/,
		);
		delete (snapshot.projection as { extra?: boolean }).extra;
		(snapshot.projection.fixture_values[0] as { extra?: boolean }).extra = true;
		expect(() => decodeProgrammerValuesSnapshot(snapshot)).toThrow(
			/declared wire field/,
		);
	});
});

describe("Programmer values mutation wire boundary", () => {
	it("encodes absolute and relative intents without client-side expansion", () => {
		expect(
			encodeProgrammerValuesActionRequest({
				requestId: "step-1",
				expectedRevision: 6,
				expectedCaptureModeRevision: 4,
				action: {
					action: "apply_intent",
					fixtureIds: [FIXTURE_ID],
					attribute: "pan",
					operation: { type: "relative_step", delta: -0.1 },
					timing: { fade: true, fadeMillis: 500, delayMillis: null },
				},
			}),
		).toEqual({
			request_id: "step-1",
			expected_revision: 6,
			expected_capture_mode_revision: 4,
			action: {
				type: "apply_intent",
				fixture_ids: [FIXTURE_ID],
				group_id: null,
				attribute: "pan",
				operation: { type: "relative_step", delta: -0.1 },
				undo_group: null,
				timing: { fade: true, fade_millis: 500, delay_millis: null },
			},
		});
	});

	it("decodes changed and no-change outcomes with different payload shapes", () => {
		expect(
			decodeProgrammerValuesActionOutcome(
				changedOutcome(),
				"request-1"),
		).toMatchObject({
			status: "changed",
			requestId: "request-1",
			revision: 7,
			captureModeRevision: 4,
			eventSequence: 19,
			projection: { revision: 7 },
		});
		const noChange = {
			...changedOutcome(),
			status: "no_change",
		};
		delete (noChange as Partial<ReturnType<typeof changedOutcome>>).projection;
		delete (noChange as Partial<ReturnType<typeof changedOutcome>>)
			.event_sequence;
		expect(
			decodeProgrammerValuesActionOutcome(noChange, "request-1"),
		).toEqual({
			status: "no_change",
			requestId: "request-1",
			correlationId: CORRELATION_ID,
			revision: 7,
			captureModeRevision: 4,
			replayed: false,
			warning: null,
		});
	});

	it("rejects mismatched requests, undeclared projection fields, and materialized no-ops", () => {
		expect(() =>
			decodeProgrammerValuesActionOutcome(
				changedOutcome(),
				"another-request"),
		).toThrow(/another-request/);
		const undeclared = changedOutcome();
		undeclared.projection = {
			...projection(),
			user_id: OTHER_SESSION_ID,
		} as typeof undeclared.projection;
		expect(() =>
			decodeProgrammerValuesActionOutcome(undeclared, "request-1"),
		).toThrow(/user_id/);
		const materializedNoOp = changedOutcome();
		materializedNoOp.status = "no_change";
		expect(() =>
			decodeProgrammerValuesActionOutcome(
				materializedNoOp,
				"request-1"),
		).toThrow(/no projection/);
	});

	it("encodes a batch as one action with nested timing", () => {
		expect(
			encodeProgrammerValuesActionRequest({
				requestId: "batch-1",
				expectedRevision: 6,
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

	it("decodes typed revision conflicts", () => {
		expect(
			decodeProgrammerValuesErrorResponse({
				kind: "conflict",
				error: "revision conflict",
				current_revision: 8,
				current_capture_mode_revision: 5,
				retryable: false,
			}),
		).toEqual({
			kind: "conflict",
			error: "revision conflict",
			currentRevision: 8,
			currentCaptureModeRevision: 5,
			retryable: false,
		});
	});

	it("rejects undeclared outcome and error fields", () => {
		expect(() =>
			decodeProgrammerValuesActionOutcome(
				{ ...changedOutcome(), extra: true },
				"request-1"),
		).toThrow(/declared wire field/);
		expect(() =>
			decodeProgrammerValuesErrorResponse({
				kind: "conflict",
				error: "revision conflict",
				retryable: false,
				extra: true,
			}),
		).toThrow(/declared wire field/);
	});
});

describe("Programmer values event wire boundary", () => {
	it("decodes the lossless delta", () => {
		expect(decodeProgrammerValuesEventMessage(valuesEvent())).toEqual({
			type: "event",
			sequence: 19,
			correlationId: CORRELATION_ID,
			change: expect.objectContaining({ revision: 7 }),
		});
	});

	it("decodes Dynamic removals with their instance track", () => {
		const event = valuesEvent();
		(event.event.payload.change.removed_dynamic_values as unknown[]).push({
			fixture_id: FIXTURE_ID,
			attribute: "intensity",
			instance_link: CORRELATION_ID,
		});
		const decoded = decodeProgrammerValuesEventMessage(event);
		expect(
			decoded.type === "event" && "change" in decoded
				? decoded.change.removedDynamicValues
				: null,
		).toEqual([
			{
				fixtureId: FIXTURE_ID,
				attribute: "intensity",
				instanceLink: CORRELATION_ID,
			},
		]);
	});

	it.each([
		[
			"another Programmer object",
			(event: ReturnType<typeof valuesEvent>) => {
				event.event.object.id = "programming-priority";
			},
		],
		[
			"change carrying an undeclared field",
			(event: ReturnType<typeof valuesEvent>) => {
				(event.event.payload.change as Record<string, unknown>).user_id =
					OTHER_SESSION_ID;
			},
		],
		[
			"desk-owned envelope",
			(event: ReturnType<typeof valuesEvent>) => {
				(event.event as { desk_id: string | null }).desk_id = FIXTURE_ID;
			},
		],
		[
			"replaceable delivery",
			(event: ReturnType<typeof valuesEvent>) => {
				event.event.delivery = "replaceable";
			},
		],
	])("rejects a %s", (_label, mutate) => {
		const event = valuesEvent();
		mutate(event);
		expect(() => decodeProgrammerValuesEventMessage(event)).toThrow(
			WireValidationError,
		);
	});

	it("rejects undeclared message, envelope, and payload fields", () => {
		const messageExtra = { ...valuesEvent(), extra: true };
		expect(() =>
			decodeProgrammerValuesEventMessage(messageExtra),
		).toThrow(/declared wire field/);

		const envelopeExtra = valuesEvent();
		(
			envelopeExtra.event as typeof envelopeExtra.event & { extra?: boolean }
		).extra = true;
		expect(() =>
			decodeProgrammerValuesEventMessage(envelopeExtra),
		).toThrow(/declared wire field/);

		const payloadExtra = valuesEvent();
		(
			payloadExtra.event.payload as typeof payloadExtra.event.payload & {
				extra?: boolean;
			}
		).extra = true;
		expect(() =>
			decodeProgrammerValuesEventMessage(payloadExtra),
		).toThrow(/declared wire field/);
	});
});
