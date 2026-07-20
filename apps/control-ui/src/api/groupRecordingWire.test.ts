import { describe, expect, it } from "vitest";
import type { GroupRecordingRequest } from "../features/groupRecording/contracts";
import {
	decodeGroupRecordErrorResponse,
	decodeGroupRecordingOutcome,
	encodeGroupRecordingRequest,
} from "./groupRecordingWire";

const REQUEST: GroupRecordingRequest = {
	requestId: "record-group-1",
	groupId: "Front Wash A / É",
	operation: "overwrite",
	expectedObjectRevision: 2,
};
const CORRELATION_ID = "33333333-3333-4333-8333-333333333333";

function storedOutcome(overrides: Record<string, unknown> = {}) {
	return {
		status: "changed",
		request_id: REQUEST.requestId,
		correlation_id: CORRELATION_ID,
		replayed: false,
		show_revision: 8,
		event_sequence: 12,
		group: {
			state: "stored",
			id: REQUEST.groupId,
			revision: 3,
			body: {
				id: REQUEST.groupId,
				name: "Front",
				color: null,
				icon: null,
				fixtures: ["fixture-2", "fixture-1"],
				derived_from: null,
				frozen_from: null,
				programming: {},
				master: 1,
				playback_fader: null,
				future: { kept: true },
			},
		},
		...overrides,
	};
}

describe("Group recording wire", () => {
	it("encodes only the action identity, operation, and expected revision", () => {
		expect(encodeGroupRecordingRequest(REQUEST)).toEqual({
			request_id: "record-group-1",
			group_id: "Front Wash A / É",
			operation: "overwrite",
			expected_object_revision: 2,
		});
	});

	it("decodes a strict stored projection and preserves body extensions", () => {
		const outcome = decodeGroupRecordingOutcome(storedOutcome(), REQUEST);

		expect(outcome).toMatchObject({
			status: "changed",
			eventSequence: 12,
			group: {
				state: "stored",
				id: "Front Wash A / É",
				revision: 3,
				object: {
					kind: "group",
					body: {
						fixtures: ["fixture-2", "fixture-1"],
						future: { kept: true },
					},
				},
			},
		});
	});

	it("accepts a losslessly preserved legacy body with only fixtures", () => {
		const outcome = decodeGroupRecordingOutcome(
			storedOutcome({
				group: {
					state: "stored",
					id: REQUEST.groupId,
					revision: 3,
					body: { fixtures: [], future: { kept: true } },
				},
			}),
			REQUEST,
		);

		expect(outcome.group.object?.body).toEqual({
			fixtures: [],
			future: { kept: true },
		});
	});

	it("normalizes an omitted legacy fixtures default to authoritative empty", () => {
		const { event_sequence: _eventSequence, ...noChange } = storedOutcome({
			status: "no_change",
			group: {
				state: "stored",
				id: REQUEST.groupId,
				revision: 2,
				body: {},
			},
		});
		const outcome = decodeGroupRecordingOutcome(noChange, REQUEST);

		if (outcome.status !== "no_change") throw new Error("Expected no-change");
		expect(outcome.group.object.body).toEqual({ fixtures: [] });
	});

	it("decodes a strict deleted projection for delete", () => {
		const request = { ...REQUEST, operation: "delete" as const };
		const outcome = decodeGroupRecordingOutcome(
			storedOutcome({
				group: {
					state: "deleted",
					id: REQUEST.groupId,
					revision: 3,
				},
			}),
			request,
		);

		expect(outcome.group).toEqual({
			state: "deleted",
			id: REQUEST.groupId,
			revision: 3,
			object: null,
		});
	});

	it("accepts either stored or deleted projection for subtract", () => {
		const request = { ...REQUEST, operation: "subtract" as const };
		expect(
			decodeGroupRecordingOutcome(storedOutcome(), request).group.state,
		).toBe("stored");
		expect(
			decodeGroupRecordingOutcome(
				storedOutcome({
					group: {
						state: "deleted",
						id: REQUEST.groupId,
						revision: 3,
					},
				}),
				request,
			).group.state,
		).toBe("deleted");
	});

	it("decodes replayed no-change without an event sequence", () => {
		const { event_sequence: _eventSequence, ...noChange } = storedOutcome({
			status: "no_change",
			replayed: true,
			group: {
				state: "stored",
				id: REQUEST.groupId,
				revision: 2,
				body: {
					id: REQUEST.groupId,
					name: "Front",
					color: null,
					icon: null,
					fixtures: [],
					derived_from: null,
					frozen_from: null,
					programming: {},
					master: 1,
					playback_fader: null,
				},
			},
		});
		const outcome = decodeGroupRecordingOutcome(noChange, REQUEST);

		expect(outcome).toMatchObject({ status: "no_change", replayed: true });
		expect("eventSequence" in outcome).toBe(false);
	});

	it("rejects an event sequence on no-change", () => {
		expect(() =>
			decodeGroupRecordingOutcome(
				{
					...storedOutcome(),
					status: "no_change",
					group: { ...storedOutcome().group, revision: 2 },
				},
				REQUEST,
			),
		).toThrow("$.event_sequence");
	});

	it("rejects deleted no-change and any no-change Delete action", () => {
		const request = { ...REQUEST, operation: "delete" as const };
		const { event_sequence: _eventSequence, ...deletedNoChange } =
			storedOutcome({
				status: "no_change",
				group: {
					state: "deleted",
					id: REQUEST.groupId,
					revision: 2,
				},
			});
		expect(() => decodeGroupRecordingOutcome(deletedNoChange, request)).toThrow(
			"$.group.state",
		);

		const { event_sequence: _storedEventSequence, ...storedNoChange } =
			storedOutcome({
				status: "no_change",
				group: {
					...storedOutcome().group,
					revision: 2,
				},
			});
		expect(() => decodeGroupRecordingOutcome(storedNoChange, request)).toThrow(
			"$.status",
		);
	});

	it("rejects malformed committed Group body fields", () => {
		const valid = storedOutcome().group.body as Record<string, unknown>;
		const malformed = [
			{ ...valid, fixtures: "fixture-1" },
			{ ...valid, id: "another-group" },
			{ ...valid, playback_fader: 256 },
			{
				...valid,
				derived_from: {
					source_group_id: "source",
					rule: { type: "every_nth", n: 0, offset: 0 },
				},
			},
			{
				...valid,
				frozen_from: {
					source_group_id: "source",
					source_revision: 2,
					captured_at: "not-a-timestamp",
				},
			},
			{
				...valid,
				programming: { intensity: { kind: "raw_dmx", value: 256 } },
			},
		];
		for (const body of malformed)
			expect(() =>
				decodeGroupRecordingOutcome(
					storedOutcome({
						group: { ...storedOutcome().group, body },
					}),
					REQUEST,
				),
			).toThrow();
	});

	it("rejects unknown outer and projection fields", () => {
		expect(() =>
			decodeGroupRecordingOutcome(
				{ ...storedOutcome(), programmer: {} },
				REQUEST,
			),
		).toThrow("$.programmer");
		expect(() =>
			decodeGroupRecordingOutcome(
				storedOutcome({
					group: { ...storedOutcome().group, selection: [] },
				}),
				REQUEST,
			),
		).toThrow("$.group.selection");
	});

	it("rejects mismatched IDs, operation states, and revisions", () => {
		expect(() =>
			decodeGroupRecordingOutcome(
				storedOutcome({
					group: { ...storedOutcome().group, id: "front-wash-a" },
				}),
				REQUEST,
			),
		).toThrow("$.group.id");
		expect(() =>
			decodeGroupRecordingOutcome(
				storedOutcome({
					group: {
						state: "deleted",
						id: REQUEST.groupId,
						revision: 3,
					},
				}),
				REQUEST,
			),
		).toThrow("$.group.state");
		expect(() =>
			decodeGroupRecordingOutcome(
				storedOutcome({
					group: { ...storedOutcome().group, revision: 4 },
				}),
				REQUEST,
			),
		).toThrow("$.group.revision");
	});

	it("strictly decodes structured action errors", () => {
		expect(
			decodeGroupRecordErrorResponse({
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
			decodeGroupRecordErrorResponse({
				kind: "conflict",
				error: "revision conflict",
				retryable: false,
				fixtures: [],
			}),
		).toThrow("$.fixtures");
	});
});
