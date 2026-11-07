import { describe, expect, it } from "vitest";
import type { PlaybackDefinition } from "../../../api/types";
import { projectPlaybackPaneFootprints } from "./paneFootprints";
import { reflowPlaybackCells } from "./paneTileGeometry";
import { playbackCellGridStyle, projectPlaybackSlots } from "./projection";

function playback(footprint: PlaybackDefinition["footprint"]): PlaybackDefinition {
	return {
		name: "Look",
		target: { type: "cue_list", cue_list_id: "look" },
		buttons: ["go_minus", "go", "flash"],
		fader: "master",
		go_activates: true,
		auto_off: true,
		xfade_millis: 0,
		footprint,
	};
}

function cells(assignments: Record<string, PlaybackDefinition>, columns = 2, pageSize = 12) {
	return projectPlaybackSlots({
		cueLists: [],
		groups: [],
		page: { number: 1, name: "Main", assignments },
		playbackLayout: null,
		columns,
		firstSlot: 401,
		pageSize,
		ignoreFootprints: true,
	});
}

describe("Playback pane footprints", () => {
	it("claims Normal cells in the final reflowed grid without changing addresses", () => {
		const source = cells({ "401": playback({ type: "pane_normal" }) }, 4, 8);
		const slots = projectPlaybackPaneFootprints(reflowPlaybackCells(source, 2));
		expect(slots.map((cell) => cell.slot)).toEqual([401, 402, 404, 406, 407, 408]);
		expect(slots[0].playback?.address).toEqual({ page: 1, position: 401 });
		expect(playbackCellGridStyle(slots[0])).toEqual({
			gridColumn: "1 / span 1",
			gridRow: "1 / span 3",
		});
	});

	it("adds one row above Taller and one column beside Wider", () => {
		const tall = projectPlaybackPaneFootprints(cells({
			"403": playback({ type: "pane_taller", upper_button: "flash" }),
		}));
		const wide = projectPlaybackPaneFootprints(cells({
			"401": playback({ type: "pane_wider", right_buttons: ["go", "pause", "flash"], right_fader: "master" }),
		}));
		expect(tall.map((cell) => cell.slot)).toEqual([402, 403, 404, 406, 408, 409, 410, 411, 412]);
		expect(playbackCellGridStyle(tall[1])).toEqual({
			gridColumn: "1 / span 1",
			gridRow: "1 / span 4",
		});
		expect(wide.map((cell) => cell.slot)).toEqual([401, 407, 408, 409, 410, 411, 412]);
		expect(playbackCellGridStyle(wide[0])).toEqual({
			gridColumn: "1 / span 2",
			gridRow: "1 / span 3",
		});
	});

	it("keeps every position when competing claims, occupied cells or bounds prevent a whole footprint", () => {
		const wide = playback({ type: "pane_wider", right_buttons: ["go", "pause", "flash"], right_fader: "master" });
		const tall = playback({ type: "pane_taller", upper_button: "flash" });
		for (const [source, anchorIndices] of [
			[cells({ "401": wide, "408": tall }), [0, 7]],
			[cells({ "401": wide, "406": playback({ type: "single_button" }) }), [0, 5]],
			[cells({ "402": wide }), [1]],
			[cells({ "401": tall }), [0]],
			[cells({ "401": playback({ type: "pane_normal" }) }, 2, 4), [0]],
		] as const) {
			const slots = projectPlaybackPaneFootprints(source);
			expect(slots.map((cell) => cell.slot)).toEqual(source.map((cell) => cell.slot));
			for (const index of anchorIndices)
				expect(slots[index].paneFootprint).toEqual({ type: "single_button" });
		}
	});
});
