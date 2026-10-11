import { describe, expect, it } from "vitest";
import {
	attachedHighlightKeys,
	attachedKeypadContentRowOffset,
	attachedPlaybackLayout,
	attachedProgrammerActionLayout,
	controlSurfaceOscPaths,
	encoderControlActions,
	feedbackPlaybackOffset,
} from "./controlSurfaceContracts";

describe("shared control-surface contracts", () => {
	it("uses explicit page and position on every Playback control", () => {
		expect(controlSurfaceOscPaths.explicitPlaybackControl(3, 21, "fader")).toBe(
			"playback/3/21/fader",
		);
	});

	it("uses canonical OSC vocabulary and Playback feedback", () => {
		expect(controlSurfaceOscPaths.programmer("record")).toBe(
			"programmer/record",
		);
		expect(controlSurfaceOscPaths.encoder(4)).toBe("encode/4");
		expect(controlSurfaceOscPaths.navigation).toBe("nav");
		expect(controlSurfaceOscPaths.highlight("previous")).toBe(
			"highlight/previous",
		);
		expect(controlSurfaceOscPaths.speedGroupButton(2)).toBe(
			"speed-group/2/button",
		);
		expect(controlSurfaceOscPaths.programmerFade("programmer")).toBe(
			"programmer/prog-fade",
		);
		expect(controlSurfaceOscPaths.programmerFade("cue")).toBe(
			"programmer/cue-fade",
		);
		expect(controlSurfaceOscPaths.programmerFade("release")).toBe(
			"programmer/release-fade",
		);
		expect(encoderControlActions).toEqual([
			"up",
			"down",
			"left",
			"right",
			"press",
		]);
		expect(feedbackPlaybackOffset(["feedback", "playback", "2", "4"])).toBe(1);
		expect(feedbackPlaybackOffset(["feedback", "page-playback", "2"])).toBe(-1);
		expect(feedbackPlaybackOffset(["feedback", "paged-playback", "2"])).toBe(
			-1,
		);
	});

	it("retains attached key order and physical slot ranges", () => {
		expect(attachedHighlightKeys).toEqual([
			{ label: "HIGH", action: "toggle", column: 1, row: 1 },
			{ label: "PREV", action: "previous", column: 2, row: 1 },
			{ label: "NEXT", action: "next", column: 3, row: 1 },
			{ label: "ALL", action: "all", column: 4, row: 1 },
		]);
		expect(attachedProgrammerActionLayout).toEqual({
			record: { column: 1, row: 1, columnSpan: 2, rowSpan: 1 },
			preload: { column: 3, row: 1, columnSpan: 2, rowSpan: 1 },
		});
		expect(attachedKeypadContentRowOffset).toBe(1);
		expect(attachedPlaybackLayout.mainSlots).toEqual([
			1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
		]);
		expect(attachedPlaybackLayout.topSlots).toEqual([
			21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38,
			39, 40,
		]);
		expect(attachedPlaybackLayout.gridButtonSlots[0]).toBe(41);
		expect(attachedPlaybackLayout.gridButtonSlots.at(-1)).toBe(90);
		expect(attachedPlaybackLayout.gridButtonSlots).toHaveLength(50);
		expect(attachedPlaybackLayout.gridPlaybackSlots).toEqual([
			91, 92, 93, 94, 95, 96,
		]);
		expect(attachedPlaybackLayout.encoderSlots).toEqual([1, 2, 3, 4, 5, 6]);
		expect(attachedPlaybackLayout.navigationEncoder).toBe(7);
		expect(attachedPlaybackLayout.speedGroups).toEqual([1, 2, 3, 4, 5]);
	});
});
