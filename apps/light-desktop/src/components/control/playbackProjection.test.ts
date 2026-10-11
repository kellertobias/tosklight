import { describe, expect, it } from "vitest";
import type { PlaybackDefinition } from "../../api/types";
import { playbackSlotAssignments } from "./playbackProjection";
const first: PlaybackDefinition = {
	name: "Front",
	target: { type: "cue_list", cue_list_id: "front" },
	buttons: ["go", "pause", "flash"],
	fader: "master",
	go_activates: true,
	auto_off: true,
	xfade_millis: 0,
};
const second: PlaybackDefinition = { ...first, name: "Back" };
describe("sparse Playback assignment projection", () => {
	it("projects consecutive positions and preserves holes without remapping definitions", () => {
		const page = {
			number: 99,
			name: "Main",
			assignments: { "401": first, "403": second },
		};
		const projected = playbackSlotAssignments(page, 401, 4);
		expect(projected).toEqual([first, undefined, second, undefined]);
		expect(projected[0]).toBe(first);
		expect(projected[2]).toBe(second);
		expect(Object.keys(page.assignments)).toEqual(["401", "403"]);
	});
	it("supports exact position1000 boundary and absent page without fabricating assignments", () => {
		expect(
			playbackSlotAssignments(
				{ number: 99, name: "Last", assignments: { "1000": first } },
				1000,
				1,
			),
		).toEqual([first]);
		expect(playbackSlotAssignments(undefined, 1000, 1)).toEqual([undefined]);
	});
});
