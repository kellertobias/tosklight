import { describe, expect, it } from "vitest";
import type {
	SpeedGroupActionRequest,
	SpeedGroupProjection,
} from "../features/speedGroupRuntime/contracts";
import {
	AUTHORITY_ID,
	CORRELATION_ID,
	DESK_ID,
	OTHER_AUTHORITY_ID,
	speedAuthority,
} from "../features/speedGroupRuntime/testFixtures";
import {
	decodeSpeedGroupActionOutcome,
	decodeSpeedGroupErrorResponse,
	decodeSpeedGroupEventMessage,
	decodeSpeedGroupSnapshot,
	encodeSpeedGroupActionRequest,
} from "./speedGroupRuntimeWire";
import { WireValidationError } from "./wireValidation";

const REQUEST: SpeedGroupActionRequest = {
	requestId: "speed-request",
	expectedAuthorityId: AUTHORITY_ID,
	expectedRevision: 4,
	expectedGroups: speedAuthority().groups,
	action: { type: "set_bpm", group: "A", bpm: 128.5 },
};

function group(id: string, overrides: Record<string, unknown> = {}) {
	return {
		group: id,
		manual_bpm: 120,
		paused: false,
		speed_master_scale: 1,
		synchronized_with: null,
		phase_origin_millis: 100,
		...overrides,
	};
}

function groups() {
	return ["A", "B", "C", "D", "E"].map((id) => group(id));
}

function outcome(overrides: Record<string, unknown> = {}) {
	return {
		request_id: REQUEST.requestId,
		correlation_id: CORRELATION_ID,
		authority_id: AUTHORITY_ID,
		revision: 5,
		applied_at_millis: 200,
		groups: [group("A", { manual_bpm: 128.5 })],
		status: "changed",
		event_sequence: 19,
		replayed: false,
		durability: "durable",
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
			object: { capability: "playback", id: "speed-groups:manual" },
			related_objects: [],
			source: { kind: "action", source: "http" },
			correlation_id: CORRELATION_ID,
			delivery: "lossless",
			payload: { type: "speed_groups_changed", change },
			...overrides,
		},
	};
}

describe("Speed Group runtime wire", () => {
	it("encodes all three typed actions with exact authority expectations", () => {
		expect(encodeSpeedGroupActionRequest(REQUEST)).toEqual({
			request_id: "speed-request",
			expected_authority_id: AUTHORITY_ID,
			expected_revision: 4,
			action: { type: "set_bpm", group: "A", bpm: 128.5 },
		});
		expect(
			encodeSpeedGroupActionRequest({
				...REQUEST,
				action: { type: "adjust_bpm", group: "B", deltaBpm: -2.5 },
			}),
		).toMatchObject({
			action: { type: "adjust_bpm", group: "B", delta_bpm: -2.5 },
		});
		expect(
			encodeSpeedGroupActionRequest({
				...REQUEST,
				action: { type: "synchronize", source: "A", target: "E" },
			}),
		).toMatchObject({
			action: { type: "synchronize", source: "A", target: "E" },
		});
	});

	it.each([
		["invalid BPM", { action: { type: "set_bpm", group: "A", bpm: 0 } }],
		["zero delta", { action: { type: "adjust_bpm", group: "A", deltaBpm: 0 } }],
		[
			"same synchronization",
			{ action: { type: "synchronize", source: "A", target: "A" } },
		],
		["malformed authority", { expectedAuthorityId: "not-a-uuid" }],
		["control request ID", { requestId: "bad\nrequest" }],
	])("rejects %s before transport", (_label, replacement) => {
		expect(() =>
			encodeSpeedGroupActionRequest({
				...REQUEST,
				...replacement,
			} as SpeedGroupActionRequest),
		).toThrow(WireValidationError);
	});

	it("strictly decodes one ordered installation snapshot", () => {
		const decoded = decodeSpeedGroupSnapshot({
			cursor: { sequence: 18 },
			projection: {
				authority_id: AUTHORITY_ID,
				revision: 4,
				groups: groups(),
			},
		});
		expect(decoded).toMatchObject({
			cursor: 18,
			projection: {
				authorityId: AUTHORITY_ID,
				revision: 4,
			},
		});
		expect(decoded.projection.groups).toHaveLength(5);
		expect(decoded.projection.groups[0]).toMatchObject({
			group: "A",
			manualBpm: 120,
		});
	});

	it.each([
		["missing group", groups().slice(0, 4)],
		["wrong order", [group("B"), group("A"), ...groups().slice(2)]],
		["duplicate", [group("A"), group("A"), ...groups().slice(2)]],
		["invalid BPM", [group("A", { manual_bpm: 1_000 }), ...groups().slice(1)]],
		[
			"self synchronization",
			[group("A", { synchronized_with: "A" }), ...groups().slice(1)],
		],
	])("rejects a snapshot with %s", (_label, values) => {
		expect(() =>
			decodeSpeedGroupSnapshot({
				cursor: { sequence: 18 },
				projection: {
					authority_id: AUTHORITY_ID,
					revision: 4,
					groups: values,
				},
			}),
		).toThrow(WireValidationError);
	});

	it("decodes changed/no-change/replay/durability without inventing an event", () => {
		expect(decodeSpeedGroupActionOutcome(outcome(), REQUEST)).toMatchObject({
			status: "changed",
			eventSequence: 19,
			groups: [{ group: "A", manualBpm: 128.5 }],
		});
		const noChange = outcome({
			revision: 4,
			status: "no_change",
			replayed: true,
			durability: "persistence_pending",
			warning: "save pending",
		}) as Record<string, unknown>;
		delete noChange.event_sequence;
		expect(decodeSpeedGroupActionOutcome(noChange, REQUEST)).toMatchObject({
			status: "no_change",
			eventSequence: null,
			replayed: true,
			durability: "persistence_pending",
			warning: "save pending",
		});
	});

	it("accepts only captured reciprocal peers in authoritative action outcomes", () => {
		const expectedGroups: SpeedGroupProjection[] = (
			["A", "B", "C", "D", "E"] as const
		).map((id) => ({
			group: id,
			manualBpm: 120,
			paused: false,
			speedMasterScale: 1,
			synchronizedWith: id === "A" ? "C" : id === "C" ? "A" : null,
			phaseOriginMillis: 100,
		}));
		const request: SpeedGroupActionRequest = {
			...REQUEST,
			expectedGroups,
			action: { type: "set_bpm", group: "C", bpm: 90 },
		};
		const authoritative = outcome({
			groups: [group("A"), group("C", { manual_bpm: 90 })],
		});

		expect(decodeSpeedGroupActionOutcome(authoritative, request)).toMatchObject(
			{ groups: [{ group: "A" }, { group: "C" }] },
		);
		expect(() =>
			decodeSpeedGroupActionOutcome(
				outcome({
					groups: [group("A"), group("B"), group("C", { manual_bpm: 90 })],
				}),
				request,
			),
		).toThrow(WireValidationError);
		expect(() =>
			decodeSpeedGroupActionOutcome(
				outcome({ groups: [group("C"), group("C")] }),
				request,
			),
		).toThrow(WireValidationError);
	});

	it.each([
		["foreign authority", { authority_id: OTHER_AUTHORITY_ID }],
		["wrong request", { request_id: "other" }],
		["wrong revision", { revision: 8 }],
		["wrong group", { groups: [group("B")] }],
		["unknown field", { projection: {} }],
	])("rejects an outcome with %s", (_label, replacement) => {
		expect(() =>
			decodeSpeedGroupActionOutcome(outcome(replacement), REQUEST),
		).toThrow(WireValidationError);
	});

	it("decodes the exact global lossless event and a gap", () => {
		const change = {
			authority_id: AUTHORITY_ID,
			revision: 5,
			applied_at_millis: 200,
			groups: [group("A", { manual_bpm: 128.5 })],
		};
		expect(decodeSpeedGroupEventMessage(event(change))).toMatchObject({
			type: "event",
			sequence: 19,
			change: { authorityId: AUTHORITY_ID, revision: 5 },
		});
		expect(
			decodeSpeedGroupEventMessage({
				type: "gap",
				gap: {
					after_sequence: 19,
					oldest_available: 25,
					latest_sequence: 30,
				},
			}),
		).toEqual({
			type: "gap",
			afterSequence: 19,
			oldestAvailable: 25,
			latestSequence: 30,
		});
	});

	it.each([
		["desk-scoped", { desk_id: DESK_ID }],
		["replaceable", { delivery: "replaceable" }],
		["runtime source", { source: { kind: "runtime" } }],
		[
			"foreign object",
			{ object: { capability: "playback", id: "playback:1" } },
		],
		[
			"wrong payload",
			{ payload: { type: "playback_runtime_changed", change: {} } },
		],
	])("rejects a %s event", (_label, replacement) => {
		const change = {
			authority_id: AUTHORITY_ID,
			revision: 5,
			applied_at_millis: 200,
			groups: [group("A")],
		};
		expect(() =>
			decodeSpeedGroupEventMessage(event(change, replacement)),
		).toThrow(WireValidationError);
	});

	it("decodes typed conflicts and rejects undeclared error fields", () => {
		expect(
			decodeSpeedGroupErrorResponse({
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
			decodeSpeedGroupErrorResponse({
				kind: "conflict",
				error: "revision conflict",
				retryable: false,
				projection: {},
			}),
		).toThrow(WireValidationError);
	});
});
