import { describe, expect, it } from "vitest";
import {
	decodeProgrammerLifecycleEventMessage,
	decodeProgrammerLifecycleSnapshot,
} from "./programmerLifecycleWire";

const PROGRAMMER_ID = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const USER_ID = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
const SESSION_ID = "cccccccc-cccc-4ccc-8ccc-cccccccccccc";
const CORRELATION_ID = "dddddddd-dddd-4ddd-8ddd-dddddddddddd";

function programmer() {
	return {
		programmer_id: PROGRAMMER_ID,
		user_id: USER_ID,
		connected: true,
		selected_fixture_count: 2,
		normal_value_count: 3,
		sessions: [{ session_id: SESSION_ID }],
	};
}

function eventMessage(delta: Record<string, unknown>) {
	return {
		type: "event",
		event: {
			sequence: 12,
			occurred_at: "2026-07-20T10:00:00Z",
			desk_id: null,
			class: "projection",
			object: { capability: "programmer", id: "programming-lifecycle" },
			related_objects: [],
			source: { kind: "action", source: "http" },
			correlation_id: CORRELATION_ID,
			delivery: "lossless",
			payload: {
				type: "programming_lifecycle_changed",
				change: { revision: 4, delta },
			},
		},
	};
}

describe("Programmer lifecycle wire decoding", () => {
	it("strictly decodes the aggregate snapshot", () => {
		expect(
			decodeProgrammerLifecycleSnapshot({
				cursor: { sequence: 11 },
				projection: { revision: 3, programmers: [programmer()] },
			}),
		).toEqual({
			cursor: 11,
			projection: {
				revision: 3,
				programmers: [
					{
						programmerId: PROGRAMMER_ID,
						userId: USER_ID,
						connected: true,
						selectedFixtureCount: 2,
						normalValueCount: 3,
						sessions: [{ sessionId: SESSION_ID }],
					},
				],
			},
		});
	});

	it("rejects undeclared Programmer content and selection identities", () => {
		const row = { ...programmer(), values: [] };
		expect(() =>
			decodeProgrammerLifecycleSnapshot({
				cursor: { sequence: 1 },
				projection: { revision: 1, programmers: [row] },
			}),
		).toThrow(/values/);

		const session = { ...programmer().sessions[0], selected: [PROGRAMMER_ID] };
		expect(() =>
			decodeProgrammerLifecycleSnapshot({
				cursor: { sequence: 1 },
				projection: {
					revision: 1,
					programmers: [{ ...programmer(), sessions: [session] }],
				},
			}),
		).toThrow(/selected/);
	});

	it("decodes one exact upsert or removal delta", () => {
		expect(
			decodeProgrammerLifecycleEventMessage(
				eventMessage({ type: "upsert", programmer: programmer() }),
			),
		).toMatchObject({
			type: "event",
			sequence: 12,
			correlationId: CORRELATION_ID,
			change: {
				revision: 4,
				delta: {
					type: "upsert",
					programmer: { programmerId: PROGRAMMER_ID, userId: USER_ID },
				},
			},
		});
		expect(
			decodeProgrammerLifecycleEventMessage(
				eventMessage({ type: "remove", programmer_id: PROGRAMMER_ID }),
			),
		).toMatchObject({
			change: {
				delta: { type: "remove", programmerId: PROGRAMMER_ID },
			},
		});
	});

	it("rejects the wrong route, delivery, or ambiguous delta", () => {
		const wrongObject = eventMessage({
			type: "remove",
			programmer_id: PROGRAMMER_ID,
		});
		wrongObject.event.object.id = `programming-values:${USER_ID}`;
		expect(() => decodeProgrammerLifecycleEventMessage(wrongObject)).toThrow(
			/programming-lifecycle/,
		);

		const wrongDelivery = eventMessage({
			type: "remove",
			programmer_id: PROGRAMMER_ID,
		});
		wrongDelivery.event.delivery = "replaceable";
		expect(() => decodeProgrammerLifecycleEventMessage(wrongDelivery)).toThrow(
			/lossless/,
		);

		expect(() =>
			decodeProgrammerLifecycleEventMessage(
				eventMessage({
					type: "remove",
					programmer_id: PROGRAMMER_ID,
					programmer: programmer(),
				}),
			),
		).toThrow(/programmer/);
	});

	it("strictly decodes cursor and gap control messages", () => {
		expect(
			decodeProgrammerLifecycleEventMessage({
				type: "ready",
				cursor: { sequence: 8 },
			}),
		).toEqual({ type: "ready", cursor: 8 });
		expect(
			decodeProgrammerLifecycleEventMessage({
				type: "gap",
				gap: {
					after_sequence: 8,
					oldest_available: 10,
					latest_sequence: 14,
				},
			}),
		).toEqual({
			type: "gap",
			afterSequence: 8,
			oldestAvailable: 10,
			latestSequence: 14,
		});
	});
});
