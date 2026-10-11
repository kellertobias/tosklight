import { describe, expect, it } from "vitest";
import {
	decodePlaybackSoloEvent,
	decodePlaybackSoloSaveOutcome,
	decodePlaybackSoloSnapshot,
	encodePlaybackSoloSaveRequest,
	PlaybackSoloProtocolError,
} from "./wire";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const OTHER_ID = "33333333-3333-4333-8333-333333333333";
const SCOPE = { showId: SHOW_ID };
const ZONE = {
	id: "stage-left",
	name: "Stage Left",
	members: [{page:1,position:1},{page:2,position:201}],
};

describe("Playback Solo Region wire", () => {
	it("decodes one show-global revisioned snapshot", () => {
		expect(
			decodePlaybackSoloSnapshot(
				{ show_id: SHOW_ID, revision: 4, regions: [ZONE] },
				SCOPE,
			),
		).toEqual({
			showId: SHOW_ID,
			revision: 4,
			regions: [
				{
					id: "stage-left",
					name: "Stage Left",
					members: [{page:1,position:1},{page:2,position:201}],
				},
			],
		});
	});

	it("rejects foreign shows, invalid pages, duplicate numbers, and overflow", () => {
		for (const [label, value] of [
			["foreign show", { show_id: OTHER_ID, revision: 0, regions: [] }],
			[
				"invalid page",
				{
					show_id: SHOW_ID,
					revision: 0,
					regions: [{ ...ZONE, members: [{page:0,position:1},{page:1,position:1}] }],
				},
			],
			[
				"duplicate number",
				{
					show_id: SHOW_ID,
					revision: 0,
					regions: [{ ...ZONE, members: [{page:1,position:1},{page:1,position:1}] }],
				},
			],
			[
				"overflow",
				{
					show_id: SHOW_ID,
					revision: 0,
					regions: [
						{
							...ZONE,
							members: [{page:1,position:1},{page:99,position:1001}],
						},
					],
				},
			],
		] as const)
			expect(
				() => decodePlaybackSoloSnapshot(value, SCOPE),
				label,
			).toThrow(PlaybackSoloProtocolError);
	});

	it("encodes only show-global edit intent", () => {
		expect(
			encodePlaybackSoloSaveRequest("request-a", 4, [
				{
					id: "stage-left",
					name: "Stage Left",
					members: [{page:1,position:1},{page:2,position:201}],
				},
			]),
		).toEqual({
			request_id: "request-a",
			expected_revision: 4,
			regions: [ZONE],
		});
	});

	it("decodes the authoritative save result", () => {
		expect(
			decodePlaybackSoloSaveOutcome(
				{
					show_id: SHOW_ID,
					revision: 5,
					regions: [ZONE],
					request_id: "request-a",
					replayed: false,
					changed: true,
				},
				SCOPE,
				"request-a",
			),
		).toMatchObject({
			showId: SHOW_ID,
			revision: 5,
			requestId: "request-a",
			regions: [{ members: [{page:1,position:1},{page:2,position:201}] }],
		});
	});

	it("decodes show-global change events", () => {
		expect(
			decodePlaybackSoloEvent({
				type: "event",
				event: {
					payload: {
						type: "playback_solo_regions_changed",
						change: { show_id: SHOW_ID, revision: 7 },
					},
				},
			}),
		).toEqual({ showId: SHOW_ID, revision: 7 });
	});
});
