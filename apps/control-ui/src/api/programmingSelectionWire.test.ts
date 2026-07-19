import { describe, expect, it } from "vitest";
import {
	decodeSelectionActionOutcome,
	encodeSelectionActionRequest,
} from "./programmingSelectionWire";

const REQUEST_ID = "selection-1";
const FIXTURE_ID = "22222222-2222-4222-8222-222222222222";
const CORRELATION_ID = "33333333-3333-4333-8333-333333333333";

function outcome() {
	return {
		request_id: REQUEST_ID,
		correlation_id: CORRELATION_ID,
		action: "gesture_applied",
		applied: 1,
		selection: {
			selected: [FIXTURE_ID],
			expression: {
				type: "sources",
				items: [{ type: "fixture", fixture_id: FIXTURE_ID }],
			},
			revision: 4,
			gesture_open: true,
		},
		event_sequence: 12,
		replayed: false,
		warning: "persistence is pending",
	};
}

describe("Programming selection wire boundary", () => {
	it("encodes semantic action variants without transport leakage", () => {
		expect(
			encodeSelectionActionRequest({
				requestId: REQUEST_ID,
				action: {
					type: "gesture",
					source: { type: "dereferenced_group", groupId: "7" },
					remove: true,
				},
			}),
		).toEqual({
			request_id: REQUEST_ID,
			action: "gesture",
			source: { type: "dereferenced_group", group_id: "7" },
			remove: true,
		});
	});

	it("decodes complete authority and retains durability warnings", () => {
		expect(decodeSelectionActionOutcome(outcome(), REQUEST_ID)).toEqual({
			requestId: REQUEST_ID,
			correlationId: CORRELATION_ID,
			action: "gesture_applied",
			applied: 1,
			selection: {
				selected: [FIXTURE_ID],
				expression: {
					type: "sources",
					items: [{ type: "fixture", fixtureId: FIXTURE_ID }],
				},
				revision: 4,
				gestureOpen: true,
			},
			eventSequence: 12,
			replayed: false,
			warning: "persistence is pending",
		});
	});

	it.each([
		["request", (value: ReturnType<typeof outcome>): void => {
			value.request_id = "other";
		}],
		["correlation", (value: ReturnType<typeof outcome>): void => {
			value.correlation_id = "bad";
		}],
		["revision", (value: ReturnType<typeof outcome>): void => {
			value.selection.revision = -1;
		}],
		["gesture", (value: ReturnType<typeof outcome>): void => {
			(value.selection as Record<string, unknown>).gesture_open = "yes";
		}],
	] as const)("rejects invalid %s authority", (_label, mutate) => {
		const value = outcome();
		mutate(value);
		expect(() => decodeSelectionActionOutcome(value, REQUEST_ID)).toThrow();
	});
});
