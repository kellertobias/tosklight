import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { CueList } from "../../api/types";
import type { TimecodeCueListClip } from "../../api/types/timecode";
import { CueListClipContents } from "./TimecodeCueClipContents";

afterEach(cleanup);
const clip: TimecodeCueListClip = {
	id: "clip",
	start_frame: 0,
	end_frame: 440,
	start_cue_id: "one",
	end_cue_id: "one",
	start_behavior: "state",
	end_behavior: "hold",
	cue_starts: [],
	in_fade_frames: 0,
	out_fade_frames: 44,
};
const list = {
	id: "list",
	name: "Audience Wash",
	mode: "sequence",
	cues: [
		{
			id: "one",
			number: "1",
			name: "Audience",
			fade_millis: 0,
			delay_millis: 0,
			out_delay_millis: 0,
			out_fade_millis: 0,
			trigger: { type: "manual" },
			changes: [],
		},
	],
} as unknown as CueList;
it("shows a single Cue, inherited In, explicit zero Out reference and separate clip ending control", () => {
	const host = document.createElement("div");
	document.body.append(host);
	const setFade = vi.fn();
	const rendered = render(
		<CueListClipContents
			cueList={{ id: list.id, name: list.name, cues: list.cues, body: list }}
			clip={clip}
			pixelsPerFrame={2}
			timingDefaults={{ sequenceFadeMillis: 1000, releaseFadeMillis: 500 }}
			selectedCueId="one"
			expanded
			inspectorHost={host}
			onSaveCueList={vi.fn().mockResolvedValue(list)}
			onSetClipFade={setFade}
		/>,
	);
	expect(
		screen.getByRole("button", { name: "Cue 1 Audience" }),
	).toBeInTheDocument();
	expect(screen.getByRole("slider", { name: "Cue 1 In fade" })).toHaveAttribute(
		"aria-valuenow",
		"44",
	);
	expect(
		screen.getByRole("slider", { name: "Cue 1 Out fade" }),
	).toHaveAttribute("aria-valuenow", "440");
	expect(
		rendered.container.querySelector(".timecode-cue-edit-rail.out.reference"),
	).not.toBeNull();
	expect(screen.getByText(/It is not played at this clip/)).toBeInTheDocument();
	expect(screen.getByText(/In inherits the desk Cue Fade/)).toBeInTheDocument();
	fireEvent.change(screen.getByLabelText("Clip Out duration"), {
		target: { value: "00:00:02.00" },
	});
	fireEvent.blur(screen.getByLabelText("Clip Out duration"));
	expect(setFade).toHaveBeenCalledExactlyOnceWith("clip", "out", 88);
	host.remove();
});
it("switching Cues discards unfinished input even when point times match", () => {
	const host = document.createElement("div");
	document.body.append(host);
	const two = {
		...list,
		cues: [
			list.cues[0],
			{
				...list.cues[0],
				id: "two",
				number: "2",
				name: "Second",
				trigger: { type: "wait" as const, delay_millis: 0 },
			},
		],
	};
	const props = {
		cueList: { id: two.id, name: two.name, cues: two.cues, body: two },
		clip: { ...clip, end_cue_id: "two" },
		pixelsPerFrame: 2,
		timingDefaults: { sequenceFadeMillis: 1000, releaseFadeMillis: 500 },
		expanded: true,
		inspectorHost: host,
	};
	const rendered = render(
		<CueListClipContents {...props} selectedCueId="one" />,
	);
	fireEvent.change(screen.getByLabelText("Trigger"), {
		target: { value: "unfinished" },
	});
	rendered.rerender(<CueListClipContents {...props} selectedCueId="two" />);
	expect(screen.getByLabelText("Trigger")).toHaveValue("00:00:00.00");
	host.remove();
});

it("expands Cue envelopes and attaches all four points to their corners", () => {
	const two = {
		...list,
		cues: [
			list.cues[0],
			{
				...list.cues[0],
				id: "two",
				number: "2",
				name: "Second",
				trigger: { type: "manual" as const },
			},
		],
	};
	const props = {
		cueList: { id: two.id, name: two.name, cues: two.cues, body: two },
		clip: {
			...clip,
			end_cue_id: "two",
			cue_starts: [{ cue_id: "two", offset_frame: 220 }],
		},
		pixelsPerFrame: 2,
		timingDefaults: { sequenceFadeMillis: 1000, releaseFadeMillis: 500 },
		onSaveCueList: vi.fn().mockResolvedValue(two),
	};
	const view = render(<CueListClipContents {...props} />);
	expect(
		view.container.querySelector(".timecode-cue-envelope"),
	).toHaveAttribute("height", "86");
	expect(screen.queryByRole("slider", { name: "Cue 1 In fade" })).toBeNull();
	view.rerender(
		<CueListClipContents {...props} selectedCueId="one" expanded />,
	);
	expect(
		view.container.querySelector(".timecode-cue-envelope"),
	).toHaveAttribute("height", "202");
	expect(screen.getByRole("slider", { name: "Cue 1 In delay" })).toHaveStyle({
		top: "226px",
	});
	expect(screen.getByRole("slider", { name: "Cue 1 In fade" })).toHaveStyle({
		top: "58px",
	});
	expect(screen.getByRole("slider", { name: "Cue 1 Out delay" })).toHaveStyle({
		top: "58px",
	});
	expect(screen.getByRole("slider", { name: "Cue 1 Out fade" })).toHaveStyle({
		top: "226px",
	});
	expect(
		screen.getByRole("button", { name: "Edit Cue 1 Audience" }),
	).toHaveStyle({ top: "234px" });
	expect(
		view.container.querySelector(".timecode-cue-fade-guide.in"),
	).not.toBeNull();
	expect(
		view.container.querySelector(".timecode-cue-fade-guide.out"),
	).not.toBeNull();
});
