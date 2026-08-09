import { describe, expect, it } from "vitest";
import type { GroupManagementRequest } from "../features/groupManagement/contracts";
import {
	decodeGroupManagementErrorResponse,
	decodeGroupManagementOutcome,
	decodeGroupSettingsSnapshot,
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
		expectedShowRevision: 7,
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
			expected_show_revision: 7,
			operation: {
				type: "update_properties",
				properties: { name: "Front wash", color: "#204060", icon: "◆" },
			},
		});
		expect(
			encodeGroupManagementRequest(
				request({
					operation: {
						type: "set_spatial_mapping",
						mapping: {
							projection: {
								anchor: { x: 0, y: 0, z: 0 },
								view_direction: { x: 0, y: 0, z: -1 },
								rotation_degrees: 0,
								preset: "top",
							},
							shape: {
								type: "grid",
								angle_degrees: 0,
								direction: "ascending",
							},
						},
					},
				}),
			).operation,
		).toMatchObject({ type: "set_spatial_mapping" });
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

	it("decodes authoritative settings provenance, ranks, and warnings", () => {
		const fixture = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
		const snapshot = decodeGroupSettingsSnapshot(
			{
				show_id: SHOW_ID,
				show_revision: 7,
				group: {
					object_id: "front",
					object_revision: 3,
					body: { name: "Front", fixtures: [fixture] },
				},
				resolved_spatial: {
					source_order: [fixture],
					effective_mapping: {
						projection: {
							anchor: { x: 0, y: 0, z: 0 },
							view_direction: { x: 0, y: 0, z: -1 },
							rotation_degrees: 0,
							preset: "top",
						},
						shape: {
							type: "grid",
							angle_degrees: 0,
							direction: "ascending",
						},
					},
					mapping_provenance: {
						type: "inherited",
						source_group_ids: ["source"],
					},
					ordered_fixture_ids: [fixture],
					projected_positions: [{ fixture_id: fixture, u: 1.25, v: -2.5 }],
					ranks: [{ fixture_id: fixture, rank: 0 }],
					rank_count: 1,
					warnings: [{ type: "missing_position", fixture_id: fixture }],
				},
			},
			"front",
		);

		expect(snapshot.group.revision).toBe(3);
		expect(snapshot.resolvedSpatial.mapping_provenance).toEqual({
			type: "inherited",
			source_group_ids: ["source"],
		});
		expect(snapshot.resolvedSpatial.ranks).toEqual([
			{ fixture_id: fixture, rank: 0 },
		]);
		expect(snapshot.resolvedSpatial.projected_positions).toEqual([
			{ fixture_id: fixture, u: 1.25, v: -2.5 },
		]);
		expect(snapshot.resolvedSpatial.warnings).toHaveLength(1);
	});

	it("decodes projection kinds while keeping legacy planar responses compatible", () => {
		const fixture = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
		const response = {
			show_id: SHOW_ID,
			show_revision: 7,
			group: {
				object_id: "front",
				object_revision: 3,
				body: { name: "Front", fixtures: [fixture] },
			},
			resolved_spatial: {
				source_order: [fixture],
				effective_mapping: {
					projection: {
						anchor: { x: 0, y: 0, z: 0 },
						view_direction: { x: 0, y: 0, z: -1 },
						rotation_degrees: 0,
						preset: null,
						kind: "cylindrical",
					},
					shape: {
						type: "grid",
						angle_degrees: 0,
						direction: "ascending",
					},
				},
				mapping_provenance: { type: "local", group_id: "front" },
				ordered_fixture_ids: [fixture],
				projected_positions: [],
				ranks: [],
				rank_count: 0,
				warnings: [],
			},
		};

		expect(
			decodeGroupSettingsSnapshot(response, "front").resolvedSpatial
				.effective_mapping?.projection.kind,
		).toBe("cylindrical");

		const { kind: _kind, ...legacyProjection } =
			response.resolved_spatial.effective_mapping.projection;
		const legacyPlanar = {
			...response,
			resolved_spatial: {
				...response.resolved_spatial,
				effective_mapping: {
					...response.resolved_spatial.effective_mapping,
					projection: legacyProjection,
				},
			},
		};
		expect(
			decodeGroupSettingsSnapshot(legacyPlanar, "front").resolvedSpatial
				.effective_mapping?.projection.kind,
		).toBeUndefined();
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
