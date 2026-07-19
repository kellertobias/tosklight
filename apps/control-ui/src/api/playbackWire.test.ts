import { describe, expect, it } from "vitest";
import {
	CUE_LIST_ID,
	cueProjection,
	DESK_ID,
	deskProjection,
} from "../features/playbackRuntime/testFixtures";
import { decodePlaybackOutcome } from "./playbackWire";

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

	it.each([
		[
			"requested address",
			{ requested: { kind: "explicit_page", page: 1, slot: "2" } },
		],
		["resolved address", { resolved: { kind: "preview", playback_number: 1 } }],
		["captured outcome", { outcome: { status: "captured", pending: "later" } }],
		["durability", { durability: "eventually" }],
		["event sequence", { event_sequence: -1 }],
		["replayed flag", { replayed: "false" }],
	])("rejects a malformed %s variant", (_label, replacement) => {
		expect(() =>
			decodePlaybackOutcome({ ...validOutcome(), ...replacement }),
		).toThrow();
	});
});
