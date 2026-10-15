import { describe, expect, it } from "vitest";
import type { PlaybackTopologyRequest } from "../features/playbackTopology/contracts";
import {
	decodePlaybackTopologyErrorResponse,
	decodePlaybackTopologyOutcome,
	encodePlaybackTopologyRequest,
} from "./playbackTopologyWire";
import type { PlaybackDefinition } from "./types";
const REQUEST_ID = "33333333-3333-4333-8333-333333333333";
const CORRELATION_ID = "44444444-4444-4444-8444-444444444444";
const CUE_LIST_ID = "22222222-2222-4222-8222-222222222222";
function playback(): PlaybackDefinition {
	return {
		name: "Front Wash",
		target: { type: "cue_list", cue_list_id: CUE_LIST_ID },
		buttons: ["toggle", "none", "none"],
		button_count: 1,
		fader: "master",
		has_fader: false,
		go_activates: true,
		auto_off: true,
		xfade_millis: 0,
		color: "#8b5cf6",
		flash_release: "release_all",
		protect_from_swap: false,
	};
}
function request(
	type: "assign_playback" | "update_playback" = "update_playback",
): PlaybackTopologyRequest {
	return {
		requestId: REQUEST_ID,
		action: {
			type,
			address: { page: 4, position: 680 },
			expectedPageRevision: 7,
			expectedPageObjectId: "page-four",
			definition: playback(),
		},
	};
}
function pageObject() {
	return {
		state: "present",
		kind: "playback_page",
		object_id: "page-four",
		object_revision: 8,
		body: { number: 4, name: "Page 4", assignments: { 680: playback() } },
	};
}
function outcome(overrides: Record<string, unknown> = {}) {
	return {
		request_id: REQUEST_ID,
		correlation_id: CORRELATION_ID,
		show_revision: 12,
		resolution: { kind: "playback", page: 4, position: 680 },
		status: "changed",
		objects: [pageObject()],
		event_sequence: 41,
		replayed: false,
		...overrides,
	};
}
describe("Unified page topology wire", () => {
	it.each([
		"assign_playback",
		"update_playback",
	] as const)("encodes %s with page ownership only", (type) => {
		expect(encodePlaybackTopologyRequest(request(type))).toEqual({
			request_id: REQUEST_ID,
			action: {
				type,
				address: { page: 4, position: 680 },
				expected_page_revision: 7,
				expected_page_object_id: "page-four",
				definition: {
					...playback(),
					footprint: { type: "normal" },
					presentation_icon: null,
					presentation_image: null,
				},
			},
		});
	});
	it("preserves zero Group seed and strips decoded target extensions", () => {
		const q = request();
		if (!("definition" in q.action)) throw Error("fixture");
		q.action.definition = {
			...playback(),
			target: { type: "group", group_id: "front", initial_master: 0 },
		};
		expect(encodePlaybackTopologyRequest(q)).toMatchObject({
			action: {
				definition: {
					target: { type: "group", group_id: "front", initial_master: 0 },
				},
			},
		});
		q.action.definition = {
			...playback(),
			target: { ...playback().target, future_target: true },
		} as unknown as PlaybackDefinition;
		expect(encodePlaybackTopologyRequest(q)).toMatchObject({
			action: { definition: { target: playback().target } },
		});
	});
	it("preserves independent wider controls", () => {
		const q = request();
		if (!("definition" in q.action)) throw Error("fixture");
		q.action.definition.footprint = {
			type: "wider",
			right_buttons: ["go_minus", "pause", "flash"],
			right_fader: "x_fade",
		};
		expect(encodePlaybackTopologyRequest(q)).toMatchObject({
			action: { definition: { footprint: q.action.definition.footprint } },
		});
	});
	it.each([
		{ page: 0, position: 1 },
		{ page: 100, position: 1 },
		{ page: 1, position: 0 },
		{ page: 1, position: 1001 },
	])("rejects out-of-contract address %j", (address) => {
		const q = request();
		if (!("address" in q.action)) throw Error("fixture");
		q.action.address = address;
		expect(() => encodePlaybackTopologyRequest(q)).toThrow("$.action.address");
	});
	it.each([
		{ expectedPageRevision: 0, expectedPageObjectId: "old" },
		{ expectedPageRevision: 7, expectedPageObjectId: null },
	])("rejects ambiguous page authority %j", (basis) => {
		const q = request();
		expect(() =>
			encodePlaybackTopologyRequest({
				...q,
				action: { ...q.action, ...basis },
			}),
		).toThrow("exact present or absent");
	});
	it("decodes page-owned assignment and exact storage identity", () => {
		expect(
			decodePlaybackTopologyOutcome(outcome(), request(), 11),
		).toMatchObject({
			status: "changed",
			resolution: { kind: "playback", page: 4, position: 680 },
			objects: [
				{
					objectId: "page-four",
					objectRevision: 8,
					body: { assignments: { 680: { name: "Front Wash" } } },
				},
			],
		});
	});
	it.each([
		{ bootstrap: {} },
		{ request_id: "other" },
		{ show_revision: 11 },
		{ resolution: { kind: "playback", page: 4, position: 681 } },
		{ objects: [] },
		{ objects: [pageObject(), pageObject()] },
		{ objects: [{ ...pageObject(), object_id: "other" }] },
		{ objects: [{ ...pageObject(), object_revision: 9 }] },
		{
			objects: [
				{
					...pageObject(),
					body: {
						...pageObject().body,
						assignments: { 680: { ...playback(), name: "wrong" } },
					},
				},
			],
		},
	])("rejects mismatched response authority %j", (override) => {
		expect(() =>
			decodePlaybackTopologyOutcome(outcome(override), request(), 11),
		).toThrow();
	});
	it("rejects retired standalone playback and split-page formats", () => {
		expect(() =>
			decodePlaybackTopologyOutcome(
				outcome({
					objects: [
						{
							state: "present",
							kind: "playback",
							object_id: "old",
							object_revision: 8,
							body: playback(),
						},
					],
				}),
				request(),
				11,
			),
		).toThrow();
		expect(() =>
			decodePlaybackTopologyOutcome(
				outcome({
					objects: [
						{
							...pageObject(),
							body: {
								number: 4,
								name: "Page",
								slots: { 1: 7 },
								virtual_playbacks: {},
							},
						},
					],
				}),
				request(),
				11,
			),
		).toThrow();
	});
	it("decodes no-change without event and rejects bumped revision", () => {
		const { event_sequence, ...base } = outcome();
		const unchanged = { ...pageObject(), object_revision: 7 };
		const value = {
			...base,
			status: "no_change",
			show_revision: 11,
			replayed: true,
			objects: [unchanged],
		};
		expect(decodePlaybackTopologyOutcome(value, request(), 11)).toMatchObject({
			status: "no_change",
			replayed: true,
		});
		expect(() =>
			decodePlaybackTopologyOutcome(
				{ ...value, objects: [pageObject()] },
				request(),
				11,
			),
		).toThrow("revision 7");
	});
	it("clears one exact assignment while preserving unrelated sparse positions", () => {
		const q: PlaybackTopologyRequest = {
			requestId: REQUEST_ID,
			action: {
				type: "clear_playback",
				address: { page: 4, position: 680 },
				expectedPageRevision: 7,
				expectedPageObjectId: "page-four",
			},
		};
		expect(
			decodePlaybackTopologyOutcome(
				outcome({
					objects: [
						{
							...pageObject(),
							body: { ...pageObject().body, assignments: { 999: playback() } },
						},
					],
				}),
				q,
				11,
			).objects,
		).toHaveLength(1);
		expect(() => decodePlaybackTopologyOutcome(outcome(), q, 11)).toThrow(
			"cleared Playback position",
		);
	});
	it("validates explicit Group target", () => {
		const q: PlaybackTopologyRequest = {
			requestId: REQUEST_ID,
			action: {
				type: "assign_group_master",
				groupObjectId: "front",
				expectedGroupRevision: 9,
				address: { page: 4, position: 680 },
				expectedPageRevision: 7,
				expectedPageObjectId: "page-four",
			},
		};
		expect(encodePlaybackTopologyRequest(q)).toMatchObject({
			action: {
				group_object_id: "front",
				expected_group_revision: 9,
				address: { page: 4, position: 680 },
			},
		});
		expect(() => decodePlaybackTopologyOutcome(outcome(), q, 11)).toThrow(
			"explicitly requested Group",
		);
	});
	it.each([
		"create_playback_page",
		"rename_playback_page",
	] as const)("encodes and decodes %s", (type) => {
		const isNew = type === "create_playback_page";
		const q: PlaybackTopologyRequest = {
			requestId: REQUEST_ID,
			action: {
				type,
				number: 4,
				name: "Act 1",
				expectedPageRevision: isNew ? 0 : 7,
				expectedPageObjectId: isNew ? null : "page-four",
			},
		};
		expect(encodePlaybackTopologyRequest(q)).toMatchObject({
			action: { type, number: 4, name: "Act 1" },
		});
		expect(
			decodePlaybackTopologyOutcome(
				outcome({
					resolution: { kind: "page", page: 4 },
					objects: [
						{
							...pageObject(),
							object_id: isNew ? "4" : "page-four",
							object_revision: isNew ? 1 : 8,
							body: { number: 4, name: "Act 1", assignments: {} },
						},
					],
				}),
				q,
				11,
			),
		).toMatchObject({ resolution: { kind: "page", page: 4 } });
	});
	it("validates exact page deletion", () => {
		const q: PlaybackTopologyRequest = {
			requestId: REQUEST_ID,
			action: {
				type: "delete_playback_page",
				number: 4,
				expectedPageRevision: 7,
				expectedPageObjectId: "page-four",
			},
		};
		expect(
			decodePlaybackTopologyOutcome(
				outcome({
					resolution: { kind: "page", page: 4 },
					objects: [
						{
							state: "deleted",
							kind: "playback_page",
							object_id: "page-four",
							object_revision: 8,
						},
					],
				}),
				q,
				11,
			).objects[0].state,
		).toBe("deleted");
	});
	it("preserves Cuelist semantic versus storage identities and history", () => {
		const body = {
			id: CUE_LIST_ID,
			name: "Main",
			priority: 0,
			mode: "sequence" as const,
			looped: false,
			cues: [],
		};
		const q: PlaybackTopologyRequest = {
			requestId: REQUEST_ID,
			action: {
				type: "save_cue_list",
				cueListId: CUE_LIST_ID,
				expectedRevision: 4,
				expectedObjectId: "stored-list",
				body,
			},
		};
		const value = outcome({
			resolution: { kind: "cue_list", cue_list_id: CUE_LIST_ID },
			objects: [
				{
					state: "present",
					kind: "cue_list",
					object_id: "stored-list",
					object_revision: 5,
					body,
				},
			],
		});
		expect(
			decodePlaybackTopologyOutcome(value, q, 11).objects[0].objectId,
		).toBe("stored-list");
		for (const type of ["undo_cue_list", "redo_cue_list"] as const)
			expect(
				encodePlaybackTopologyRequest({
					requestId: REQUEST_ID,
					action: {
						type,
						cueListId: CUE_LIST_ID,
						expectedRevision: 4,
						expectedObjectId: "stored-list",
					},
				}),
			).toMatchObject({ action: { type, expected_object_id: "stored-list" } });
	});
	it("encodes intent Dynamic assignment and decodes nested stored reference", () => {
		const fallback = {
			id: "dynamic-one",
			pool_number: 1,
			name: "Beam Show PWM",
			revision: 4,
			lanes: [],
			random_groups: [],
			target_binding: { type: "live_group", group_id: "group-beams" },
			spatial_mapping: {
				projection: { type: "inherit" },
				shape: { type: "inherit" },
			},
		};
		const assignment = {
			dynamic_id: "dynamic-one",
			last_known_pool_number: 1,
			embedded_fallback: fallback,
			revision: 2,
			target_scope: null,
			fader_mode: "size_and_master",
			priority: 0,
			activation_override: null,
			resume_policy: "follow_dynamic",
			local_speed_multiplier: { numerator: 1, denominator: 1 },
			learned_duration_millis: null,
			crossfade_non_intensity: false,
			auto_off_at_zero: false,
			auto_off_flash_release: false,
			auto_off_full_control: true,
		};
		const q = request();
		if (!("definition" in q.action)) throw Error("fixture");
		q.action.definition = {
			...playback(),
			target: { type: "dynamic", assignment },
		} as unknown as PlaybackDefinition;
		expect(encodePlaybackTopologyRequest(q)).toMatchObject({
			action: { definition: { target: { assignment } } },
		});
		const { dynamic_id, last_known_pool_number, embedded_fallback, ...timing } =
			assignment;
		const wireDefinition = {
			...q.action.definition,
			target: {
				type: "dynamic",
				assignment: {
					...timing,
					dynamic: {
						dynamic_id,
						last_known_pool_number,
						embedded_fallback: {
							definition: { ...embedded_fallback, spatial_mapping: undefined },
						},
					},
				},
			},
		};
		expect(
			decodePlaybackTopologyOutcome(
				outcome({
					objects: [
						{
							...pageObject(),
							body: {
								...pageObject().body,
								assignments: { 680: wireDefinition },
							},
						},
					],
				}),
				q,
				11,
			),
		).toMatchObject({
			objects: [
				{
					body: {
						assignments: {
							680: { target: { assignment: { embedded_fallback: fallback } } },
						},
					},
				},
			],
		});
	});
	it("decodes strict conflict metadata", () => {
		expect(
			decodePlaybackTopologyErrorResponse({
				kind: "conflict",
				error: "stale Page",
				current_revision: 15,
				current_related_revision: 9,
				retryable: false,
			}),
		).toEqual({
			kind: "conflict",
			error: "stale Page",
			currentRevision: 15,
			currentRelatedRevision: 9,
			retryable: false,
		});
		expect(() =>
			decodePlaybackTopologyErrorResponse({
				kind: "internal",
				error: "failed",
				retryable: false,
				hidden: true,
			}),
		).toThrow();
	});
});


it.each(["assign_playback", "update_playback"] as const)(
	"preserves saved Macro and Timecode identities in actual %s JSON",
	(operation) => {
		for (const target of [
			{ type: "macro", macro_id: "bdd36e24-730f-4090-a6bc-43fcadff5add" },
			{ type: "timecode", timecode_id: "d63061ad-6aad-4701-8442-b6f0d2aca93b" },
		] as const) {
			const value = request(operation);
			if (!("definition" in value.action)) throw Error("fixture");
			value.action.definition.target = target;
			const wire = JSON.parse(JSON.stringify(encodePlaybackTopologyRequest(value)));
			expect(wire.action.definition.target).toEqual(target);
			expect(wire.action.expected_page_revision).toBe(7);
			expect(wire.request_id).toBe(REQUEST_ID);
		}
	},
);

it.each(["macro", "timecode"] as const)(
	"rejects missing or malformed %s identity before sending an assignment",
	(type) => {
		for (const identity of [undefined, "", "bad\nidentity", 5]) {
			const value = request("assign_playback");
			if (!("definition" in value.action)) throw Error("fixture");
			value.action.definition.target = {
				type,
				[`${type}_id`]: identity,
			} as PlaybackDefinition["target"];
			expect(() => encodePlaybackTopologyRequest(value)).toThrow();
		}
	},
);
