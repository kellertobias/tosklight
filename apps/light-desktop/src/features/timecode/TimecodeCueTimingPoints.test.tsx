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
function mount(trigger: "follow" | "wait", pixelsPerFrame: number, endFrame = clip.end_frame) {
	const placedClip = { ...clip, end_frame: endFrame };
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
	const selected = cueClipTimingRows(placedClip, cueList, defaults).rows[0];
	const begin = vi.fn();
	const view = render(
		<CueTimingPoints
			selected={selected}
			cueList={cueList}
			timingDefaults={defaults}
			clip={placedClip}
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
function expectSeparated(targets: HTMLElement[]) {
	for (let i = 0; i < targets.length; i++)
		for (let j = i + 1; j < targets.length; j++) {
			const dx = Math.abs(
				Number.parseFloat(targets[i].style.left) -
					Number.parseFloat(targets[j].style.left),
			);
			const dy = Math.abs(
				Number.parseFloat(targets[i].style.top) -
					Number.parseFloat(targets[j].style.top),
			);
			expect(dx >= 44 || dy >= 44).toBe(true);
		}
}
it("separates FOLLOW-zero fade controls horizontally without moving their time anchors", () => {
	const { selected, begin } = mount("follow", 2);
	const inEnd = screen.getByRole("slider", { name: "Cue 1 In fade" });
	const outStart = screen.getByRole("slider", { name: "Cue 1 Out delay" });
	expect(inEnd.dataset.timeAnchorX).toBe(outStart.dataset.timeAnchorX);
	expect(inEnd).toHaveStyle({ top: "32px" });
	expect(outStart).toHaveStyle({ top: "32px" });
	expectSeparated([inEnd, outStart]);
	expect(outStart.querySelector(".timecode-cue-point-leader")).not.toBeNull();
	fireEvent.pointerDown(inEnd, { pointerId: 1, clientX: 100 });
	expect(begin.mock.lastCall?.slice(1)).toEqual([selected, "in", "end"]);
	fireEvent.pointerDown(outStart, { pointerId: 2, clientX: 100 });
	expect(begin.mock.lastCall?.slice(1)).toEqual([selected, "out", "start"]);
});
it("keeps nearby upper and lower controls independently reachable inside the short envelope", () => {
	mount("wait", 0.5);
	const targets = [
		"Trigger",
		"In fade",
		"Out delay",
		"In delay",
		"Out fade",
	].map((name) => screen.getByRole("slider", { name: `Cue 1 ${name}` }));
	expectSeparated(targets);
	expect(
		targets.every((target) => ["32px", "104px"].includes(target.style.top)),
	).toBe(true);
	expect(screen.getByRole("slider", { name: "Cue 1 In fade" })).toHaveAttribute(
		"data-time-anchor-x",
		"22",
	);
});
it("keeps Trigger distinct from every fade target when all fade times are zero", () => {
	const cueList = {
		id: "list",
		name: "Instant",
		mode: "sequence",
		cues: [
			{
				id: "one",
				number: "1",
				name: "First",
				fade_millis: 0,
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
				fade_millis: 0,
				delay_millis: 0,
				trigger: { type: "wait", delay_millis: 0 },
				changes: [],
			},
		],
	} as unknown as CueList;
	const timingDefaults = { sequenceFadeMillis: 0, releaseFadeMillis: 0 };
	const selected = cueClipTimingRows(clip, cueList, timingDefaults).rows[0];
	render(
		<CueTimingPoints
			selected={selected}
			cueList={cueList}
			timingDefaults={timingDefaults}
			clip={clip}
			position={(frame) => frame * 2}
			pixelsPerFrame={2}
			triggerPreview={null}
			first
			triggerBounds={null}
			canMoveTrigger
			setTriggerPreview={vi.fn()}
			placeTrigger={vi.fn()}
			saving={false}
			editable
			begin={vi.fn()}
			commit={vi.fn()}
		/>,
	);
	const targets = [
		"Trigger",
		"In fade",
		"Out delay",
		"In delay",
		"Out fade",
	].map((name) => screen.getByRole("slider", { name: `Cue 1 ${name}` }));
	expect(targets.map((target) => target.style.top)).toEqual([
		"32px",
		"32px",
		"32px",
		"104px",
		"104px",
	]);
	expect(targets.every((target) => target.dataset.timeAnchorX === "0")).toBe(
		true,
	);
	expectSeparated(targets);
	expect(targets[1].querySelector(".timecode-cue-point-leader")).not.toBeNull();
});

it("keeps a short single-Cue Out reference clear of In controls", () => {
	const timingDefaults = { sequenceFadeMillis: 0, releaseFadeMillis: 0 };
	const smallClip = { ...clip, end_frame: 4, end_cue_id: "one" };
	const cueList = {
		id: "list",
		name: "Instant",
		mode: "sequence",
		cues: [
			{
				id: "one",
				number: "1",
				name: "First",
				fade_millis: 0,
				delay_millis: 0,
				out_delay_millis: 0,
				out_fade_millis: 0,
				trigger: { type: "manual" },
				changes: [],
			},
		],
	} as unknown as CueList;
	const selected = cueClipTimingRows(smallClip, cueList, timingDefaults)
		.rows[0];
	render(
		<CueTimingPoints
			selected={selected}
			cueList={cueList}
			timingDefaults={timingDefaults}
			clip={smallClip}
			position={(frame) => frame * 3}
			pixelsPerFrame={3}
			triggerPreview={null}
			first
			triggerBounds={null}
			canMoveTrigger
			setTriggerPreview={vi.fn()}
			placeTrigger={vi.fn()}
			saving={false}
			editable
			begin={vi.fn()}
			commit={vi.fn()}
		/>,
	);
	const points = ["Trigger", "In fade", "In delay", "Out delay", "Out fade"]
		.map((name) => screen.getByRole("slider", { name: `Cue 1 ${name}` }))
		.map((target) => ({
			x:
				Number.parseFloat(target.style.left) +
				(target.closest(".reference") ? 6 : 0),
			y:
				Number.parseFloat(target.style.top) +
				(target.closest(".reference") ? 24 : 0),
		}));
	for (let i = 0; i < points.length; i++)
		for (let j = i + 1; j < points.length; j++)
			expect(
				Math.abs(points[i].x - points[j].x) >= 44 ||
					Math.abs(points[i].y - points[j].y) >= 44,
			).toBe(true);
	expect(screen.getByText("Out · not executed")).toBeInTheDocument();
});

it("re-packs a scrolled final Out reference and resets its scroll on Cue selection", () => {
	const body = {
		id: "list",
		name: "Reference",
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
		],
	} as unknown as CueList;
	const oneClip = { ...clip, end_cue_id: "one" };
	const selected = cueClipTimingRows(oneClip, body, defaults).rows[0];
	const props = {
		cueList: body,
		timingDefaults: defaults,
		clip: oneClip,
		position: (frame: number) => frame * 2,
		pixelsPerFrame: 2,
		triggerPreview: null,
		first: true,
		triggerBounds: null,
		canMoveTrigger: true,
		setTriggerPreview: vi.fn(),
		placeTrigger: vi.fn(),
		saving: false,
		editable: true,
		begin: vi.fn(),
		commit: vi.fn(),
	};
	const view = render(<CueTimingPoints {...props} selected={selected} />);
	const rail = view.container.querySelector<HTMLElement>(
		".timecode-cue-edit-rail.reference",
	)!;
	rail.scrollLeft = 374;
	fireEvent.scroll(rail);
	expect(rail).toHaveAttribute("data-reference-scroll", "374");
	const inEnd = screen.getByRole("slider", { name: "Cue 1 In fade" });
	const outStart = screen.getByRole("slider", { name: "Cue 1 Out delay" });
	const actualOutX =
		440 + Number.parseFloat(outStart.style.left) - rail.scrollLeft;
	expect(
		Math.abs(actualOutX - Number.parseFloat(inEnd.style.left)),
	).toBeGreaterThanOrEqual(44);
	const next = {
		...selected,
		cue: { ...selected.cue, id: "next", number: "2" },
	};
	view.rerender(<CueTimingPoints {...props} selected={next} />);
	expect(rail.scrollLeft).toBe(0);
	expect(rail).toHaveAttribute("data-reference-scroll", "0");
});

it("finds a guaranteed free target beyond densely occupied divider candidates", () => {
	const body = {
		id: "list",
		name: "Dense",
		mode: "sequence",
		cues: [
			{
				id: "one",
				number: "1",
				name: "First",
				fade_millis: 1000,
				delay_millis: 9091,
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
				trigger: { type: "follow", delay_millis: 0 },
				changes: [],
			},
		],
	} as unknown as CueList;
	const longClip = { ...clip, end_frame: 1200 };
	const selected = cueClipTimingRows(longClip, body, defaults).rows[0];
	const dividers = Array.from({ length: 43 }, (_, index) => index * 24);
	render(
		<CueTimingPoints
			selected={selected}
			cueList={body}
			timingDefaults={defaults}
			clip={longClip}
			position={(frame) => frame}
			pixelsPerFrame={1}
			triggerPreview={null}
			first
			triggerBounds={null}
			canMoveTrigger
			setTriggerPreview={vi.fn()}
			placeTrigger={vi.fn()}
			saving={false}
			editable
			dividerFrames={dividers}
			begin={vi.fn()}
			commit={vi.fn()}
		/>,
	);
	const inStart = screen.getByRole("slider", { name: "Cue 1 In delay" });
	const x = Number.parseFloat(inStart.style.left);
	expect(x).toBeGreaterThan(Math.max(...dividers));
	for (const divider of dividers)
		expect(Math.abs(x - divider)).toBeGreaterThanOrEqual(44);
	expectSeparated([
		inStart,
		screen.getByRole("slider", { name: "Cue 1 Out fade" }),
	]);
});

it("reserves the inset end-grip touch center instead of the clip boundary", () => {
	mount("follow", 2, 22);
	const endGrip = { x: 22 * 2 - 22, y: 134 };
	for (const name of ["In delay", "In fade", "Out delay", "Out fade"]) {
		const handle = screen.getByRole("slider", { name: `Cue 1 ${name}` });
		const x = Number.parseFloat(handle.style.left);
		const y = Number.parseFloat(handle.style.top);
		expect(Math.abs(x - endGrip.x) >= 44 || Math.abs(y - endGrip.y) >= 44).toBe(true);
	}
});
