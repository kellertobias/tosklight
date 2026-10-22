import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { CueList } from "../../api/types";
import type { TimecodeCueListClip } from "../../api/types/timecode";
import { cueClipTimingRows } from "./cueClipTiming";
import { CueTimingPoints } from "./TimecodeCueTimingPoints";
afterEach(cleanup);
const defaults = { sequenceFadeMillis: 1000, releaseFadeMillis: 500 };
const clip: TimecodeCueListClip = {
	id: "clip",
	start_frame: 0,
	end_frame: 440,
	start_cue_id: "one",
	end_cue_id: "two",
	start_behavior: "state",
	end_behavior: "hold",
	cue_starts: [],
	in_fade_frames: 0,
	out_fade_frames: 0,
};
function mount(trigger: "follow" | "wait", pixelsPerFrame: number) {
	const cueList = {
		id: "list",
		name: "Opening",
		mode: "sequence",
		cues: [
			{
				id: "one",
				number: "1",
				name: "First",
				fade_millis: 1000,
				delay_millis: 0,
				out_delay_millis: 0,
				out_fade_millis: 0,
				trigger: { type: "manual" },
				changes: [],
			},
			{
				id: "two",
				number: "2",
				name: "Next",
				fade_millis: 1000,
				delay_millis: 0,
				trigger: { type: trigger, delay_millis: 0 },
				changes: [],
			},
		],
	} as unknown as CueList;
	const selected = cueClipTimingRows(clip, cueList, defaults).rows[0];
	const begin = vi.fn();
	const view = render(
		<CueTimingPoints
			selected={selected}
			cueList={cueList}
			timingDefaults={defaults}
			clip={clip}
			position={(frame) => frame * pixelsPerFrame}
			pixelsPerFrame={pixelsPerFrame}
			triggerPreview={null}
			first
			triggerBounds={null}
			canMoveTrigger
			setTriggerPreview={vi.fn()}
			placeTrigger={vi.fn()}
			saving={false}
			editable
			begin={begin}
			commit={vi.fn()}
		/>,
	);
	return { view, selected, begin };
}
it("separates ordinary FOLLOW-zero In-end and Out-start touch targets at the same time", () => {
	const { selected, begin } = mount("follow", 2);
	const inEnd = screen.getByRole("slider", { name: "Cue 1 In fade" });
	const outStart = screen.getByRole("slider", { name: "Cue 1 Out delay" });
	expect(inEnd.style.left).toBe(outStart.style.left);
	expect(inEnd).toHaveStyle({ top: "58px" });
	expect(outStart).toHaveStyle({ top: "106px" });
	expect(outStart.querySelector(".timecode-cue-point-leader")).toHaveStyle({
		height: "48px",
	});
	fireEvent.pointerDown(inEnd, { pointerId: 1, clientX: 100 });
	expect(begin.mock.lastCall?.slice(1)).toEqual([selected, "in", "end"]);
	fireEvent.pointerDown(outStart, { pointerId: 2, clientX: 100 });
	expect(begin.mock.lastCall?.slice(1)).toEqual([selected, "out", "start"]);
});
it("separates nearby upper points and coincident lower points without moving their times", () => {
	mount("wait", 0.5);
	expect(screen.getByRole("slider", { name: "Cue 1 In delay" })).toHaveStyle({
		left: "0px",
		top: "178px",
	});
	expect(screen.getByRole("slider", { name: "Cue 1 Out fade" })).toHaveStyle({
		left: "0px",
		top: "226px",
	});
	expect(screen.getByRole("slider", { name: "Cue 1 Out delay" })).toHaveStyle({
		top: "106px",
	});
	expect(screen.getByRole("slider", { name: "Cue 1 In fade" })).toHaveStyle({
		left: "22px",
		top: "58px",
	});
});
