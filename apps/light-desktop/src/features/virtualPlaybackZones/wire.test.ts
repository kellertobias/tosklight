import { describe, expect, it } from "vitest";
import { MAX_VIRTUAL_PLAYBACK_ZONE_NUMBER } from "./contracts";
import {
	decodeVirtualPlaybackZonesEvent,
	decodeVirtualPlaybackZonesSaveOutcome,
	decodeVirtualPlaybackZonesSnapshot,
	encodeVirtualPlaybackZonesSaveRequest,
	VirtualPlaybackZonesProtocolError,
} from "./wire";

const SHOW_ID = "11111111-1111-4111-8111-111111111111";
const OTHER_ID = "33333333-3333-4333-8333-333333333333";
const SCOPE = { showId: SHOW_ID };
const ZONE = {
	id: "stage-left",
	name: "Stage Left",
	playback_numbers: [1001, 1301],
};

describe("Virtual Playback exclusion-zone wire", () => {
	it("decodes one show-global revisioned snapshot", () => {
		expect(
			decodeVirtualPlaybackZonesSnapshot(
				{ show_id: SHOW_ID, revision: 4, zones: [ZONE] },
				SCOPE,
			),
		).toEqual({
			showId: SHOW_ID,
			revision: 4,
			zones: [
				{
					id: "stage-left",
					name: "Stage Left",
					playbackNumbers: [1001, 1301],
				},
			],
		});
	});

	it("rejects foreign shows, physical numbers, duplicate numbers, and overflow", () => {
		for (const [label, value] of [
			["foreign show", { show_id: OTHER_ID, revision: 0, zones: [] }],
			[
				"physical number",
				{
					show_id: SHOW_ID,
					revision: 0,
					zones: [{ ...ZONE, playback_numbers: [1000, 1001] }],
				},
			],
			[
				"duplicate number",
				{
					show_id: SHOW_ID,
					revision: 0,
					zones: [{ ...ZONE, playback_numbers: [1001, 1001] }],
				},
			],
			[
				"overflow",
				{
					show_id: SHOW_ID,
					revision: 0,
					zones: [
						{
							...ZONE,
							playback_numbers: [1001, MAX_VIRTUAL_PLAYBACK_ZONE_NUMBER + 1],
						},
					],
				},
			],
		] as const)
			expect(
				() => decodeVirtualPlaybackZonesSnapshot(value, SCOPE),
				label,
			).toThrow(VirtualPlaybackZonesProtocolError);
	});

	it("encodes only show-global edit intent", () => {
		expect(
			encodeVirtualPlaybackZonesSaveRequest("request-a", 4, [
				{
					id: "stage-left",
					name: "Stage Left",
					playbackNumbers: [1001, 1301],
				},
			]),
		).toEqual({
			request_id: "request-a",
			expected_revision: 4,
			zones: [ZONE],
		});
	});

	it("decodes the authoritative save result", () => {
		expect(
			decodeVirtualPlaybackZonesSaveOutcome(
				{
					show_id: SHOW_ID,
					revision: 5,
					zones: [ZONE],
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
			zones: [{ playbackNumbers: [1001, 1301] }],
		});
	});

	it("decodes show-global change events", () => {
		expect(
			decodeVirtualPlaybackZonesEvent({
				type: "event",
				event: {
					payload: {
						type: "virtual_playback_exclusion_zones_changed",
						change: { show_id: SHOW_ID, revision: 7 },
					},
				},
			}),
		).toEqual({ showId: SHOW_ID, revision: 7 });
	});
});
