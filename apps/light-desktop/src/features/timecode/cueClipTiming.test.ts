import { describe, expect, it } from "vitest";
import type { Cue, CueList } from "../../api/types";
import type { TimecodeCueListClip } from "../../api/types/timecode";
import {
	cueClipTimingRows,
	cueFadeFrameBounds,
	cueFadeEditFrameBounds,
	cueTransitionFrameBounds,
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
	cue_starts: [],
	in_fade_frames: 0,
	out_fade_frames: 0,
};
const defaults = { sequenceFadeMillis: 3_000, releaseFadeMillis: 4_000 };

describe("Cue List clip timing", () => {
	it("marks only otherwise valid In-end overflow as repairable by extending the clip", () => {
		const short = { ...clip, end_frame: 60, end_cue_id: "cue-1" };
		const first = {
			...cue("cue-1", "1", { type: "manual" }),
			fade_millis: 0,
		};
		const row = cueClipTimingRows(short, cueList([first]), defaults).rows[0];
		expect(row.clipEndOverflow).toBe(true);
		expect(row.diagnostic).toContain("timing extends outside");
		const repaired = cueClipTimingRows(
			{ ...short, end_frame: 220 }, cueList([first]), defaults,
		).rows[0];
		expect(repaired.clipEndOverflow).toBeUndefined();
		expect(repaired.diagnostic).toBeUndefined();
		const invalid = cueClipTimingRows(short, cueList([{
			...first,
			trigger: { type: "link", cue_id: "missing", delay_millis: 0 },
		}]), defaults).rows[0];
		expect(invalid.diagnostic).toContain("missing Cue");
		expect(invalid.clipEndOverflow).toBeUndefined();
	});

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
		expect(rows[0]?.handoverFrame).toBe(110);
		expect(rows[2]?.outFadeReference).toBe(true);
		expect(rows[1]?.diagnostic).toContain("not reached");
	});

	it("lets the lane own the transition point of a manual GO Cue", () => {
		const list = cueList([
			cue("cue-1", "1", { type: "manual" }),
			cue("cue-2", "2", { type: "manual" }),
			cue("cue-3", "3", { type: "follow", delay_millis: 0 }),
		]);
		const { rows } = cueClipTimingRows(clip, list, defaults);
		expect(rows).toHaveLength(3);
		expect(rows.every((row) => row.diagnostic === undefined)).toBe(true);
		// Without a placed point the manual Cue follows its predecessor's completion.
		expect(rows[1]).toMatchObject({ startFrame: 88, transition: "default" });
		expect(rows[2]?.transition).toBeUndefined();

		const placed = cueClipTimingRows(
			{ ...clip, cue_starts: [{ cue_id: "cue-2", offset_frame: 88 }] },
			list,
			defaults,
		).rows;
		expect(placed[1]).toMatchObject({ startFrame: 132, transition: "placed" });
		expect(placed[2]?.startFrame).toBe(176);
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

	it("uses the latest In or Out completion for Follow, Link and default manual starts", () => {
		for (const trigger of [
			{ type: "follow", delay_millis: 500 },
			{ type: "manual" },
		] as Cue["trigger"][]) {
			const first = { ...cue("cue-1", "1", { type: "manual" }),
				out_delay_millis: 500, out_fade_millis: 2_000 };
			const list = cueList([first, cue("cue-2", "2", trigger)]);
			const rows = cueClipTimingRows({ ...clip, end_cue_id: "cue-2" }, list, defaults).rows;
			expect(rows[1].startFrame).toBe(44 + 110 + (trigger.type === "follow" ? 22 : 0));
			first.trigger = { type: "link", cue_id: "cue-2", delay_millis: 500 };
			expect(cueClipTimingRows({ ...clip, end_cue_id: "cue-2" }, list, defaults).rows[1].startFrame).toBe(176);
		}
	});

	it("keeps derived successors movable but stops a manual trigger before a later fixed Cue", () => {
		const list = cueList([
			cue("cue-1", "1", { type: "manual" }),
			cue("cue-2", "2", { type: "manual" }),
			cue("cue-3", "3", { type: "follow", delay_millis: 500 }),
			cue("cue-4", "4", { type: "timecode", frame: 300 }),
		]);
		const placedClip = { ...clip, end_cue_id: "cue-4" };
		expect(cueTransitionFrameBounds(placedClip, list, defaults, "cue-2"))
			.toEqual({ minimum: 45, maximum: 277 });
		expect(cueTransitionFrameBounds(placedClip, list, defaults, "cue-1")).toBeNull();
		expect(cueTransitionFrameBounds(placedClip, list, defaults, "cue-3")).toBeNull();
		list.cues[3].trigger = { type: "manual" };
		placedClip.cue_starts = [{ cue_id: "cue-4", offset_frame: 300 }];
		expect(cueTransitionFrameBounds(placedClip, list, defaults, "cue-2"))
			.toEqual({ minimum: 45, maximum: 277 });
	});

	it("bounds Out points from handover and permits the final shared reference beyond clip end", () => {
		const list = cueList([
			cue("cue-1", "1", { type: "manual" }),
			cue("cue-2", "2", { type: "wait", delay_millis: 4_000 }),
		]);
		const targetClip = { ...clip, end_cue_id: "cue-2" };
		const [first, last] = cueClipTimingRows(targetClip, list, defaults).rows;
		expect(cueFadeFrameBounds(first, targetClip, "out", "start").minimum).toBe(first.handoverFrame);
		expect(cueWithDraggedFade(first.cue, first, targetClip, "out", "start", first.handoverFrame - 1).error).toContain("hands over");
		expect(last.outFadeReference).toBe(true);
		expect(cueFadeFrameBounds(last, targetClip, "out", "end").maximum).toBeGreaterThan(targetClip.end_frame);
		expect(cueWithDraggedFade(last.cue, last, targetClip, "out", "end", targetClip.end_frame + 88).cue)
			.toMatchObject({ out_fade_millis: 2_000, out_delay_millis: 0 });
	});

	it("shows inherited In timing honestly while preserving an explicit zero Out fade", () => {
		const source = { ...cue("cue-1", "1", { type: "manual" }), fade_millis: 0, out_fade_millis: 0 };
		const targetClip = { ...clip, end_cue_id: "cue-1" };
		const row = cueClipTimingRows(targetClip, cueList([source]), defaults).rows[0];
		expect(row.inFade.endFrame - row.inFade.startFrame).toBe(132);
		expect(row.outFade.endFrame).toBe(row.outFade.startFrame);
		expect(cueWithDraggedFade(source, row, targetClip, "out", "end", row.outFade.startFrame).cue?.out_fade_millis).toBe(0);
	});

	it("bounds fade edits before derived successors cross a fixed trigger", () => {
		const list = cueList([
			cue("cue-1", "1", { type: "manual" }),
			cue("cue-2", "2", { type: "follow", delay_millis: 0 }),
			cue("cue-3", "3", { type: "timecode", frame: 132 }),
		]);
		const row = cueClipTimingRows(clip, list, defaults).rows[0];
		expect(cueFadeEditFrameBounds(clip, list, defaults, row, "in", "end"))
			.toEqual({ minimum: 44, maximum: 176 });
		list.cues[1].trigger = { type: "manual" };
		list.cues[2].trigger = { type: "manual" };
		const placedClip = { ...clip, cue_starts: [{ cue_id: "cue-3", offset_frame: 132 }] };
		expect(cueFadeEditFrameBounds(placedClip, list, defaults, row, "in", "end"))
			.toEqual({ minimum: 44, maximum: 176 });
	});

	it("bounds Out completion edits that would move a Follow past a fixed Cue", () => {
		const list = cueList([
			cue("cue-1", "1", { type: "manual" }),
			cue("cue-2", "2", { type: "follow", delay_millis: 0 }),
			cue("cue-3", "3", { type: "timecode", frame: 132 }),
		]);
		const row = cueClipTimingRows(clip, list, defaults).rows[0];
		const bounds = cueFadeEditFrameBounds(clip, list, defaults, row, "out", "end");
		expect(bounds?.maximum).toBe(row.handoverFrame + 132);
	});

	it("excludes an invalid zero-In inheritance endpoint while allowing adjacent explicit timing", () => {
		const list = cueList([
			cue("cue-1", "1", { type: "manual" }),
			cue("cue-2", "2", { type: "follow", delay_millis: 0 }),
			cue("cue-3", "3", { type: "timecode", frame: 88 }),
		]);
		const row = cueClipTimingRows(clip, list, defaults).rows[0];
		expect(cueFadeEditFrameBounds(clip, list, defaults, row, "in", "end"))
			.toEqual({ minimum: 45, maximum: 132 });
		expect(cueFadeEditFrameBounds(clip, list, defaults, row, "in", "start"))
			.toEqual({ minimum: 44, maximum: 87 });
	});

	it("round-trips every fixed-44fps frame through stored milliseconds", () => {
		for (let frame = 0; frame <= 440; frame += 1)
			expect(millisToTimecodeFrames(timecodeFramesToMillis(frame))).toBe(frame);
	});
	it("hands a Cue over to the next one, leaving no gap on either band", () => {
		const list = cueList([
			cue("cue-1", "1", { type: "follow", delay_millis: 0 }),
			cue("cue-2", "2", { type: "wait", delay_millis: 4_000 }),
			cue("cue-3", "3", { type: "wait", delay_millis: 4_000 }),
		]);
		const { rows } = cueClipTimingRows(clip, list, defaults);
		// Cue 2 waits four seconds after Cue 1 starts, so Cue 1 is held for longer than it faded.
		expect(rows.map((row) => row.handoverFrame)).toEqual([220, 396, 440]);
		for (const [index, row] of rows.entries()) {
			// The lower band runs in delay, in fade, then content, with nothing left over.
			expect(row.inFade.endFrame).toBeLessThanOrEqual(row.handoverFrame);
			// The upper band runs content, out delay, out fade, all measured from the handover.
			expect(row.outFade.startFrame).toBeGreaterThanOrEqual(row.handoverFrame);
			const next = rows[index + 1];
			if (next) expect(next.startFrame).toBe(row.handoverFrame);
		}
	});

	it("measures the out delay from the handover rather than from the Cue's own start", () => {
		const list = cueList([
			cue("cue-1", "1", { type: "follow", delay_millis: 0 }),
			cue("cue-2", "2", { type: "wait", delay_millis: 4_000 }),
		]);
		list.cues[0] = { ...list.cues[0], out_delay_millis: 1_000 } as Cue;
		const { rows } = cueClipTimingRows(
			{ ...clip, end_cue_id: "cue-2" },
			list,
			defaults,
		);
		// Cue 1 starts at 44 and hands over at 220; a one-second out delay releases from 220.
		expect(rows[0]?.outFade.startFrame).toBe(264);
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
