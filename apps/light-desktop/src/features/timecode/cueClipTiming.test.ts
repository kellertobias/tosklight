import { describe, expect, it } from "vitest";
import type { Cue, CueList } from "../../api/types";
import type { TimecodeCueListClip } from "../../api/types/timecode";
import {
	cueClipTimingRows,
	cueWithDraggedFade,
	millisToTimecodeFrames,
	timecodeFramesToMillis,
} from "./cueClipTiming";

const clip: TimecodeCueListClip = {
	id: "clip",
	start_frame: 44,
	end_frame: 440,
	start_cue_id: "cue-1",
	end_cue_id: "cue-3",
	start_behavior: "state",
	end_behavior: "release",
};
const defaults = { sequenceFadeMillis: 3_000, releaseFadeMillis: 4_000 };

describe("Cue List clip timing", () => {
	it("uses incoming-Cue Wait/Follow semantics and clip-relative Timecode frames", () => {
		const list = cueList([
			cue("cue-1", "1", { type: "follow", delay_millis: 9_000 }),
			cue("cue-2", "2", { type: "wait", delay_millis: 1_000 }),
			cue("cue-3", "3", { type: "timecode", frame: 220 }),
		]);
		const { rows } = cueClipTimingRows(clip, list, defaults);
		expect(rows.map((row) => row.startFrame)).toEqual([44, 88, 264]);
		expect(rows[1]?.inFade).toEqual({ startFrame: 88, endFrame: 132 });
	});

	it("uses incoming Follow completion but lets a source Link choose the destination", () => {
		const followList = cueList([
			cue("cue-1", "1", { type: "follow", delay_millis: 9_000 }),
			cue("cue-2", "2", { type: "follow", delay_millis: 500 }),
		]);
		expect(
			cueClipTimingRows(
				{ ...clip, end_cue_id: "cue-2" },
				followList,
				defaults,
			).rows.map((row) => row.startFrame),
		).toEqual([44, 110]);

		const linkList = cueList([
			cue("cue-1", "1", {
				type: "link",
				cue_id: "cue-3",
				delay_millis: 500,
			}),
			cue("cue-2", "2", { type: "follow", delay_millis: 0 }),
			cue("cue-3", "3", { type: "follow", delay_millis: 0 }),
		]);
		const { rows } = cueClipTimingRows(clip, linkList, defaults);
		expect(rows[2]?.startFrame).toBe(110);
		expect(rows[1]?.diagnostic).toContain("not reached");
	});

	it("keeps unsupported manual Cues visible with an actionable diagnostic", () => {
		const list = cueList([
			cue("cue-1", "1", { type: "manual" }),
			cue("cue-2", "2", { type: "manual" }),
			cue("cue-3", "3", { type: "follow", delay_millis: 0 }),
		]);
		const { rows } = cueClipTimingRows(clip, list, defaults);
		expect(rows).toHaveLength(3);
		expect(rows[1]?.diagnostic).toContain("manual GO");
		expect(rows[2]?.diagnostic).toContain("not reached");
	});

	it("moves fade starts with a fixed end and clears Out links on explicit edits", () => {
		const source = {
			...cue("cue-1", "1", { type: "follow", delay_millis: 0 }),
			delay_millis: 1_000,
			fade_millis: 2_000,
			out_delay_link: "in_fade" as const,
			out_fade_link: "release" as const,
		};
		const list = cueList([
			source,
			cue("cue-2", "2", { type: "follow", delay_millis: 0 }),
			cue("cue-3", "3", { type: "follow", delay_millis: 0 }),
		]);
		const row = cueClipTimingRows(clip, list, defaults).rows[0];
		if (!row) throw new Error("expected first Cue timing row");
		const moved = cueWithDraggedFade(
			source,
			row,
			clip,
			"out",
			"start",
			row.outFade.startFrame + 44,
		);
		expect(moved.cue).toMatchObject({
			out_delay_millis: 3_000,
			out_fade_millis: 3_000,
		});
		expect(moved.cue?.out_delay_link).toBeUndefined();
		expect(moved.cue?.out_fade_link).toBeUndefined();
		expect(
			cueWithDraggedFade(
				source,
				row,
				clip,
				"in",
				"end",
				row.inFade.startFrame - 1,
			).error,
		).toContain("cannot cross");
	});

	it("round-trips every fixed-44fps frame through stored milliseconds", () => {
		for (let frame = 0; frame <= 440; frame += 1)
			expect(millisToTimecodeFrames(timecodeFramesToMillis(frame))).toBe(frame);
	});
});

function cue(id: string, number: string, trigger: Cue["trigger"]): Cue {
	return {
		id,
		number,
		name: `Cue ${number}`,
		fade_millis: 1_000,
		delay_millis: 0,
		trigger,
		changes: [],
	};
}

function cueList(cues: Cue[]): CueList {
	return {
		id: "list",
		name: "Opening",
		mode: "sequence",
		priority: 0,
		looped: false,
		cues,
	};
}
