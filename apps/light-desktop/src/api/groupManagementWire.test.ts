import { describe, expect, it } from "vitest";
import type { GroupManagementRequest } from "../features/groupManagement/contracts";
import {
	decodeGroupManagementErrorResponse,
	decodeGroupManagementOutcome,
	encodeGroupManagementRequest,
} from "./groupManagementWire";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const CORRELATION_ID = "33333333-3333-4333-8333-333333333333";

function request(
	overrides: Partial<GroupManagementRequest> = {},
): GroupManagementRequest {
	return {
		requestId: "manage-1",
		groupId: "front",
		expectedObjectRevision: 1,
		operation: {
			type: "update_properties",
			properties: { name: "Front wash", color: "#204060", icon: "◆" },
		},
		...overrides,
	};
}

function changed(overrides: Record<string, unknown> = {}) {
	return {
		status: "changed",
		request_id: "manage-1",
		correlation_id: CORRELATION_ID,
		replayed: false,
		show_id: SHOW_ID,
		show_revision: 8,
		group: {
			object_id: "front",
			object_revision: 2,
			body: { name: "Front wash", fixtures: ["fixture-1"] },
		},
		show_event_sequence: 12,
		...overrides,
	};
}

describe("group management wire", () => {
	it("encodes each operation without inventing scope", () => {
		expect(encodeGroupManagementRequest(request())).toEqual({
			request_id: "manage-1",
			group_id: "front",
			expected_object_revision: 1,
			operation: {
				type: "update_properties",
				properties: { name: "Front wash", color: "#204060", icon: "◆" },
			},
		});
		expect(
			encodeGroupManagementRequest(request({ operation: { type: "undo" } }))
				.operation,
		).toEqual({ type: "undo" });
		expect(
			encodeGroupManagementRequest(
				request({
					operation: {
						type: "refresh_frozen",
						expectedSource: {
							sourceGroupId: "source",
							expectedSourceRevision: 3,
						},
					},
				}),
			).operation,
		).toEqual({
			type: "refresh_frozen",
			expected_source: {
				source_group_id: "source",
				expected_source_revision: 3,
			},
		});
	});

	it("rejects a blank Group name before any request is sent", () => {
		expect(() =>
			encodeGroupManagementRequest(
				request({
					operation: {
						type: "update_properties",
						properties: { name: "  ", color: null, icon: null },
					},
				}),
			),
		).toThrow();
	});

	it("decodes a changed outcome and its lossless body", () => {
		const outcome = decodeGroupManagementOutcome(
			changed({
				group: {
					object_id: "front",
					object_revision: 2,
					body: {
						name: "Front wash",
						fixtures: ["fixture-1", "fixture-2"],
						future_extension: { retain: true },
					},
				},
			}),
			request(),
		);

		expect(outcome.status).toBe("changed");
		expect(outcome.group.revision).toBe(2);
		expect(outcome.group.object.body).toMatchObject({
			fixtures: ["fixture-1", "fixture-2"],
			future_extension: { retain: true },
		});
		expect(outcome.persistenceWarning).toBeNull();
	});

	it("rejects undeclared response fields", () => {
		expect(() =>
			decodeGroupManagementOutcome(changed({ unexpected: true }), request()),
		).toThrow();
		expect(() =>
			decodeGroupManagementOutcome(
				changed({
					group: {
						object_id: "front",
						object_revision: 2,
						body: {},
						unexpected: true,
					},
				}),
				request(),
			),
		).toThrow();
	});

	it("rejects a foreign request, Group, or revision", () => {
		expect(() =>
			decodeGroupManagementOutcome(changed({ request_id: "other" }), request()),
		).toThrow();
		expect(() =>
			decodeGroupManagementOutcome(
				changed({
					group: { object_id: "other", object_revision: 2, body: {} },
				}),
				request(),
			),
		).toThrow();
		expect(() =>
			decodeGroupManagementOutcome(
				changed({
					group: { object_id: "front", object_revision: 5, body: {} },
				}),
				request(),
			),
		).toThrow();
	});

	it("requires a no-change outcome to omit an event sequence", () => {
		const noChange = decodeGroupManagementOutcome(
			{
				status: "no_change",
				request_id: "manage-1",
				correlation_id: CORRELATION_ID,
				replayed: true,
				show_id: SHOW_ID,
				show_revision: 8,
				group: { object_id: "front", object_revision: 1, body: {} },
			},
			request(),
		);
		expect(noChange).toMatchObject({ status: "no_change", replayed: true });
		expect(() =>
			decodeGroupManagementOutcome(
				changed({ status: "no_change", group: undefined }),
				request(),
			),
		).toThrow();
	});

	it("decodes an error response with both revision hints", () => {
		expect(
			decodeGroupManagementErrorResponse({
				kind: "conflict",
				error: "stale Group object revision",
				current_revision: 7,
				current_related_revision: 9,
				retryable: false,
			}),
		).toEqual({
			kind: "conflict",
			error: "stale Group object revision",
			currentRevision: 7,
			currentRelatedRevision: 9,
			retryable: false,
		});
		expect(() =>
			decodeGroupManagementErrorResponse({
				kind: "made_up",
				error: "x",
				retryable: false,
			}),
		).toThrow();
	});
});
