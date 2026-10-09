import { describe, expect, it } from "vitest";
import {
	decodeProgrammerPreloadValuesActionOutcome,
	decodeProgrammerPreloadValuesErrorResponse,
	decodeProgrammerPreloadValuesEventMessage,
	decodeProgrammerPreloadValuesSnapshot,
	encodeProgrammerPreloadValuesActionRequest,
} from "./programmerPreloadValuesWire";
import { decodeProgrammerPreloadValuesProjection } from "./programmerPreloadValuesWireProjection";
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
		dynamic_values: [],
	};
}

function snapshot() {
	return { cursor: { sequence: 18 }, projection: projection() };
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

function preloadEvent() {
	return {
		type: "event",
		event: {
			sequence: 19,
			occurred_at: "2026-07-20T12:00:00Z",
			desk_id: null,
			class: "projection",
			object: {
				capability: "programmer",
				id: "programming-preload-values",
			},
			related_objects: [],
			source: { kind: "action", source: "http" },
			correlation_id: CORRELATION_ID,
			delivery: "replaceable",
			payload: {
				type: "programming_preload_values_changed",
				change: { projection: projection() },
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
	it("decodes only pending values with timing and order", () => {
		expect(decodeProgrammerPreloadValuesSnapshot(snapshot())).toEqual({
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

	it("rejects a snapshot carrying an undeclared field", () => {
		const candidate = snapshot();
		(candidate.projection as Record<string, unknown>).user_id =
			OTHER_SESSION_ID;
		expect(() => decodeProgrammerPreloadValuesSnapshot(candidate)).toThrow(
			/user_id/,
		);
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
			expect(() => decodeProgrammerPreloadValuesSnapshot(candidate)).toThrow(
				/declared wire field/,
			);
		}
	});
});

describe("Preload Programmer values mutation wire", () => {
	it("encodes one server-owned linked-value intent", () => {
		expect(
			encodeProgrammerPreloadValuesActionRequest({
				requestId: "intent-1",
				expectedPreloadRevision: 6,
				expectedCaptureModeRevision: 4,
				action: {
					action: "apply_intent",
					fixtureIds: [FIXTURE_ID],
					attribute: "color.red",
					operation: {
						type: "absolute_set",
						value: { kind: "normalized", value: 0.8 },
					},
					undoGroup: "encoder-gesture",
					timing: { fade: true, fadeMillis: 500, delayMillis: null },
				},
			}),
		).toEqual({
			request_id: "intent-1",
			expected_revision: 6,
			expected_capture_mode_revision: 4,
			action: {
				type: "apply_intent",
				fixture_ids: [FIXTURE_ID],
				group_id: null,
				attribute: "color.red",
				operation: {
					type: "absolute_set",
					value: { kind: "normalized", value: 0.8 },
				},
				undo_group: "encoder-gesture",
				timing: {
					fade: true,
					fade_millis: 500,
					delay_millis: null,
				},
			},
		});
	});

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
			decodeProgrammerPreloadValuesActionOutcome(changedOutcome(), "request-1"),
		).toMatchObject({
			status: "changed",
			requestId: "request-1",
			preloadRevision: 7,
			captureModeRevision: 4,
			eventSequence: 19,
			projection: { revision: 7 },
		});
		const noChange = changedOutcome();
		noChange.status = "no_change";
		delete (noChange as Partial<ReturnType<typeof changedOutcome>>).projection;
		delete (noChange as Partial<ReturnType<typeof changedOutcome>>)
			.event_sequence;
		expect(
			decodeProgrammerPreloadValuesActionOutcome(noChange, "request-1"),
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

	it("rejects an undeclared action projection field, materialized no-op, and extras", () => {
		const undeclared = changedOutcome();
		(undeclared.projection as Record<string, unknown>).user_id =
			OTHER_SESSION_ID;
		expect(() =>
			decodeProgrammerPreloadValuesActionOutcome(undeclared, "request-1"),
		).toThrow(/user_id/);
		const noOp = changedOutcome();
		noOp.status = "no_change";
		expect(() =>
			decodeProgrammerPreloadValuesActionOutcome(noOp, "request-1"),
		).toThrow(/no projection/);
		expect(() =>
			decodeProgrammerPreloadValuesActionOutcome(
				{ ...changedOutcome(), extra: true },
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
	it("decodes the replaceable projection object", () => {
		expect(decodeProgrammerPreloadValuesEventMessage(preloadEvent())).toEqual({
			type: "event",
			sequence: 19,
			correlationId: CORRELATION_ID,
			projection: expect.objectContaining({ revision: 7 }),
		});
	});

	it("rejects another Programmer object and an undeclared projection field", () => {
		const foreignObject = preloadEvent();
		foreignObject.event.object.id = "programming-values";
		expect(() =>
			decodeProgrammerPreloadValuesEventMessage(foreignObject),
		).toThrow(WireValidationError);
		const foreignProjection = preloadEvent();
		(
			foreignProjection.event.payload.change.projection as Record<
				string,
				unknown
			>
		).user_id = OTHER_SESSION_ID;
		expect(() =>
			decodeProgrammerPreloadValuesEventMessage(foreignProjection),
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
				decodeProgrammerPreloadValuesEventMessage(candidate),
			).toThrow(/declared wire field/);
		}
	});

	it("strictly decodes cursor and gap control messages", () => {
		expect(
			decodeProgrammerPreloadValuesEventMessage({
				type: "ready",
				cursor: { sequence: 3 },
			}),
		).toEqual({ type: "ready", cursor: 3 });
		expect(() =>
			decodeProgrammerPreloadValuesEventMessage({
				type: "ready",
				cursor: { sequence: 3, extra: true },
			}),
		).toThrow(/declared wire field/);
		expect(() =>
			decodeProgrammerPreloadValuesEventMessage({
				type: "gap",
				gap: {
					after_sequence: 3,
					oldest_available: 5,
					latest_sequence: 9,
					extra: true,
				},
			}),
		).toThrow(/declared wire field/);
	});
});

describe("Preload Programmer semantic component edit intents", () => {
	const GROUP_ID = "99999999-9999-4999-8999-999999999999";
	const POINT_ID = "dddddddd-dddd-4ddd-8ddd-dddddddddddd";
	const timing = { fade: false, fadeMillis: null, delayMillis: 75 };
	const wireTiming = { fade: false, fade_millis: null, delay_millis: 75 };
	const anglePan = [
		{ kind: "activate_angles" },
		{
			kind: "scalar",
			component: { kind: "pan" },
			operation: { kind: "set", value: { kind: "spread", value: [-90, 90] } },
		},
	] as const;
	const targetXyz = [
		{ kind: "target", reference: { kind: "origin" } },
		{
			kind: "scalar",
			component: { kind: "target_x" },
			operation: { kind: "relative", value: 1 },
		},
		{
			kind: "scalar",
			component: { kind: "target_y" },
			operation: { kind: "relative", value: 0 },
		},
		{
			kind: "scalar",
			component: { kind: "target_z" },
			operation: { kind: "set", value: { kind: "value", value: 3 } },
		},
	] as const;
	const pointTarget = [
		{ kind: "target", reference: { kind: "point", point_id: POINT_ID } },
	] as const;
	const whiteBlendUv = [
		{
			kind: "scalar",
			component: { kind: "color", component: "uv" },
			operation: { kind: "relative", value: -0.2 },
		},
		{
			kind: "scalar",
			component: { kind: "color", component: "white_blend" },
			operation: { kind: "set", value: { kind: "value", value: 1 } },
		},
	] as const;
	const focus = [
		{
			kind: "scalar",
			component: { kind: "focus" },
			operation: { kind: "set", value: { kind: "value", value: 0.2 } },
		},
	] as const;
	const zoom = [
		{
			kind: "scalar",
			component: { kind: "zoom" },
			operation: { kind: "relative", value: 4 },
		},
	] as const;

	function request(
		requestId: string,
		attribute: string,
		edits: readonly unknown[],
		address: { fixtureIds: string[]; groupId?: string },
	) {
		return encodeProgrammerPreloadValuesActionRequest({
			requestId,
			expectedPreloadRevision: 21,
			expectedCaptureModeRevision: 8,
			action: {
				action: "apply_intent",
				...address,
				attribute,
				operation: { type: "component_edits", edits: edits as never },
				undoGroup: null,
				timing,
			},
		});
	}

	it.each([
		["Angle activation plus Pan", "position", anglePan],
		["Target reference plus XYZ", "position", targetXyz],
		["Target point reference", "position", pointTarget],
		["UV and White Blend", "color", whiteBlendUv],
		["Focus", "focus", focus],
		["Zoom", "zoom", zoom],
	])("preserves ordered %s edits with Preload revisions", (_name, attribute, edits) => {
		expect(
			request("preload-edit", attribute, edits, { fixtureIds: [FIXTURE_ID] }),
		).toEqual({
			request_id: "preload-edit",
			expected_revision: 21,
			expected_capture_mode_revision: 8,
			action: {
				type: "apply_intent",
				fixture_ids: [FIXTURE_ID],
				group_id: null,
				attribute,
				operation: { type: "component_edits", edits },
				undo_group: null,
				timing: wireTiming,
			},
		});
	});

	it("keeps live Group addressing distinct from fixture addressing", () => {
		expect(
			request("preload-group", "position", anglePan, {
				fixtureIds: [],
				groupId: GROUP_ID,
			}).action,
		).toMatchObject({
			fixture_ids: [],
			group_id: GROUP_ID,
			operation: { type: "component_edits", edits: anglePan },
		});
	});

	it("still encodes a relative intent", () => {
		const encoded = encodeProgrammerPreloadValuesActionRequest({
			requestId: "step-1",
			expectedPreloadRevision: 1,
			expectedCaptureModeRevision: 1,
			action: {
				action: "apply_intent",
				fixtureIds: [FIXTURE_ID],
				attribute: "dimmer",
				operation: { type: "relative_step", delta: 0.1 },
				timing,
			},
		});
		expect(encoded.action).toMatchObject({
			operation: { type: "relative_step", delta: 0.1 },
		});
	});

	it("rejects malformed component edits before transport", () => {
		expect(() =>
			request(
				"bad",
				"zoom",
				[
					{
						kind: "scalar",
						component: { kind: "zoom" },
						operation: { kind: "absolute", value: 1 },
					},
				],
				{ fixtureIds: [FIXTURE_ID] },
			),
		).toThrow(/\$\.action\.operation\.edits\[0\]\.operation\.kind/);
		expect(() =>
			encodeProgrammerPreloadValuesActionRequest({
				requestId: "bad-op",
				expectedPreloadRevision: 1,
				expectedCaptureModeRevision: 1,
				action: {
					action: "apply_intent",
					fixtureIds: [FIXTURE_ID],
					attribute: "zoom",
					operation: { type: "merge" } as never,
					timing,
				},
			}),
		).toThrow(WireValidationError);
	});
});

describe("Preload Programmer values gesture finish wire", () => {
	it("encodes the generated finish_gesture action in the canonical envelope", () => {
		expect(
			encodeProgrammerPreloadValuesActionRequest({
				requestId: "finish-1",
				expectedPreloadRevision: 6,
				expectedCaptureModeRevision: 4,
				action: {
					action: "finish_gesture",
					attribute: "pan",
					undoGroup: "gesture-1",
				},
			}),
		).toEqual({
			request_id: "finish-1",
			expected_revision: 6,
			expected_capture_mode_revision: 4,
			action: {
				type: "finish_gesture",
				attribute: "pan",
				undo_group: "gesture-1",
			},
		});
	});

	it("rejects a Finish without its original identity or envelope", () => {
		const request = {
			requestId: "finish-1",
			expectedPreloadRevision: 6,
			expectedCaptureModeRevision: 4,
			action: {
				action: "finish_gesture" as const,
				attribute: "pan",
				undoGroup: "gesture-1",
			},
		};
		for (const invalid of [
			{ ...request, action: { ...request.action, undoGroup: "" } },
			{ ...request, action: { ...request.action, attribute: "" } },
			{ ...request, requestId: "" },
			{ ...request, expectedPreloadRevision: -0.5 },
		])
			expect(() => encodeProgrammerPreloadValuesActionRequest(invalid)).toThrow(
				WireValidationError,
			);
	});
});

describe("Preload inspection release projection", () => {
	it("retains unsorted dynamic release and ordered Group ownership, with absent/empty backward compatibility", () => {
		const legacy = projection();
		expect(
			decodeProgrammerPreloadValuesProjection(legacy, "$").groupReleaseValues,
		).toBeUndefined();
		expect(
			decodeProgrammerPreloadValuesProjection(
				{ ...legacy, group_release_values: [] },
				"$",
			),
		).toEqual(decodeProgrammerPreloadValuesProjection(legacy, "$"));
		const result = decodeProgrammerPreloadValuesProjection(
			{
				...legacy,
				dynamic_values: [9, 2].map((programmer_order) => ({
					fixture_id: FIXTURE_ID,
					attribute: "intensity",
					programmer_order,
					changed_at_millis: 12,
					value: { type: "release" },
				})),
				group_release_values: [8, 1].map((programmer_order) => ({
					group_id: "3",
					attribute: "intensity",
					programmer_order,
					changed_at_millis: 12,
				})),
			},
			"$",
		);
		expect(result.dynamicValues?.map((row) => row.programmerOrder)).toEqual([
			9, 2,
		]);
		expect(
			result.groupReleaseValues?.map((row) => row.programmerOrder),
		).toEqual([8, 1]);
		expect(() =>
			decodeProgrammerPreloadValuesProjection(
				{
					...legacy,
					group_release_values: [
						{
							group_id: "3",
							attribute: "intensity",
							programmer_order: 1,
							changed_at_millis: 0,
							unknown: true,
						},
					],
				},
				"$",
			),
		).toThrow(WireValidationError);
	});
});
