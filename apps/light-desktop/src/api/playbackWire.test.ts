import { describe, expect, it } from "vitest";
import {
	CUE_LIST_ID,
	cueProjection,
	DESK_ID,
	deskProjection,
	GROUP_ID,
	groupProjection,
} from "../features/playbackRuntime/testFixtures";
import {
	decodePlaybackEventMessage,
	decodePlaybackOutcome,
} from "./playbackWire";

function validOutcome() {
	return {
		request_id: "request-1",
		correlation_id: "55555555-5555-4555-8555-555555555555",
		requested: { kind: "playback", playback_number: 1 },
		resolved: {
			kind: "playback",
			playback_number: 1,
			page: 1,
			slot: 1,
		},
		outcome: { status: "applied" },
		durability: "durable",
		projection: cueProjection(),
		related: [],
		desk: deskProjection(),
		event_sequence: 12,
		desk_event_sequence: null,
		replayed: false,
	};
}

describe("Playback wire validation", () => {
	it("decodes every requested address shape into validated data", () => {
		for (const [requested, resolved] of [
			[
				{ kind: "cue_list", cue_list_id: CUE_LIST_ID },
				{ kind: "cue_list", cue_list_id: CUE_LIST_ID },
			],
			[
				{ kind: "group", group_id: GROUP_ID },
				{ kind: "group", group_id: GROUP_ID, playback_number: null },
			],
			[
				{ kind: "playback", playback_number: 1 },
				{ kind: "playback", playback_number: 1, page: null, slot: null },
			],
			[
				{ kind: "current_page", slot: 2 },
				{ kind: "playback", playback_number: 2, page: 3, slot: 2 },
			],
			[
				{ kind: "explicit_page", page: 3, slot: 2 },
				{ kind: "playback", playback_number: 2, page: 3, slot: 2 },
			],
		] as const) {
			const decoded = decodePlaybackOutcome({
				...validOutcome(),
				requested,
				resolved,
				untrusted_extra: DESK_ID,
			});
			expect(decoded.requested).toEqual(requested);
			expect(decoded.resolved).toEqual(resolved);
			expect("untrusted_extra" in decoded).toBe(false);
		}
	});

	it("decodes an assigned Group outcome without accepting a forged request mapping", () => {
		const projection = groupProjection(GROUP_ID, 0.6, 12);
		const decoded = decodePlaybackOutcome({
			...validOutcome(),
			requested: { kind: "group", group_id: GROUP_ID },
			resolved: {
				kind: "group",
				group_id: GROUP_ID,
				playback_number: 12,
			},
			projection,
		});

		expect(decoded.requested).toEqual({ kind: "group", group_id: GROUP_ID });
		expect(decoded.resolved).toEqual({
			kind: "group",
			group_id: GROUP_ID,
			playback_number: 12,
		});
		expect(decoded.projection).toEqual(projection);
	});

	it("requires both exact Group and mapped Playback routes for assigned events", () => {
		const projection = groupProjection(GROUP_ID, 0.4, 12);
		const event = (relatedObjects: Array<{ capability: string; id: string }>) => ({
			type: "event",
			event: {
				sequence: 13,
				object: { capability: "playback", id: `group:${GROUP_ID}` },
				related_objects: relatedObjects,
				payload: {
					type: "playback_runtime_changed",
					change: { projection, transition: null },
				},
			},
		});

		expect(
			decodePlaybackEventMessage(
				event([{ capability: "playback", id: "playback:12" }]),
			),
		).toMatchObject({ type: "event", payload: { projection } });
		expect(() => decodePlaybackEventMessage(event([]))).toThrow(
			/Playback route playback:12/,
		);
	});

	it.each(["", "front\n", "x".repeat(257)])(
		"rejects malformed opaque Group ID %j",
		(groupId) => {
			expect(() =>
				decodePlaybackOutcome({
					...validOutcome(),
					requested: { kind: "group", group_id: groupId },
				}),
			).toThrow(/group_id/);
		},
	);

	it("decodes related authoritative projections and their exact event sequences", () => {
		const first = cueProjection(2, 3);
		const second = cueProjection(3, 2);
		const decoded = decodePlaybackOutcome({
			...validOutcome(),
			related: [
				{
					projection: first,
					event_sequence: 10,
					untrusted_extra: DESK_ID,
				},
				{ projection: second, event_sequence: 11 },
			],
		});

		expect(decoded.related).toEqual([
			{ projection: first, event_sequence: 10 },
			{ projection: second, event_sequence: 11 },
		]);
	});

	it.each([
		[
			"a foreign show",
			{
				projection: {
					...cueProjection(2),
					scope: {
						...cueProjection(2).scope,
						show_id: "99999999-9999-4999-8999-999999999999",
					},
				},
				event_sequence: 11,
			},
		],
		[
			"a foreign show revision",
			{
				projection: {
					...cueProjection(2),
					scope: {
						...cueProjection(2).scope,
						show_revision: cueProjection(2).scope.show_revision + 1,
					},
				},
				event_sequence: 11,
			},
		],
	])("rejects related outcomes from %s", (_label, related) => {
		expect(() =>
			decodePlaybackOutcome({ ...validOutcome(), related: [related] }),
		).toThrow(/related\[0\]\.projection\.scope/);
	});

	it.each([
		["no high-water", [11], null],
		["duplicate sequences", [11, 11], 12],
		["decreasing sequences", [11, 10], 12],
		["a sequence above the high-water", [11, 13], 12],
	])("rejects related outcomes with %s", (_label, sequences, highWater) => {
		expect(() =>
			decodePlaybackOutcome({
				...validOutcome(),
				event_sequence: highWater,
				related: sequences.map((eventSequence, index) => ({
					projection: cueProjection(index + 2),
					event_sequence: eventSequence,
				})),
			}),
		).toThrow();
	});

	it.each([
		[
			"requested address",
			{ requested: { kind: "explicit_page", page: 1, slot: "2" } },
		],
		["resolved address", { resolved: { kind: "preview", playback_number: 1 } }],
		["captured outcome", { outcome: { status: "captured", pending: "later" } }],
		["durability", { durability: "eventually" }],
		["related outcomes", { related: null }],
		[
			"related projection",
			{ related: [{ projection: {}, event_sequence: 11 }] },
		],
		[
			"related event sequence",
			{ related: [{ projection: cueProjection(2), event_sequence: -1 }] },
		],
		["event sequence", { event_sequence: -1 }],
		["replayed flag", { replayed: "false" }],
	])("rejects a malformed %s variant", (_label, replacement) => {
		expect(() =>
			decodePlaybackOutcome({ ...validOutcome(), ...replacement }),
		).toThrow();
	});
});
