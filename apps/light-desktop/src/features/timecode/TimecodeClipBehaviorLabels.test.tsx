import { cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, it } from "vitest";
import type { TimecodeCueListClip } from "../../api/types/timecode";
import { ClipBehaviorLabels } from "./TimecodeClipBehaviorLabels";
import { CueListClipStatus } from "./TimecodeCueClipContents";
afterEach(cleanup);
const clip: TimecodeCueListClip = { id: "clip", start_frame: 0, end_frame: 440, start_cue_id: "a", end_cue_id: "b", start_behavior: "state", end_behavior: "hold", cue_starts: [], in_fade_frames: 0, out_fade_frames: 0 };
it("labels the configured policies at the two clip edges", () => {
	const view = render(<ClipBehaviorLabels clip={clip} />);
	expect(screen.getByLabelText("Start behavior: State Start")).toHaveTextContent("State Start");
	expect(screen.getByLabelText("End behavior: Hold")).toHaveTextContent("Hold");
	view.rerender(<ClipBehaviorLabels clip={{ ...clip, start_behavior: "cue", end_behavior: "release", out_fade_frames: 44 }} />);
	expect(screen.getByLabelText("Start behavior: Cue Start")).toHaveTextContent("Cue Start");
	expect(screen.getByLabelText("End behavior: Release")).toHaveTextContent("Release · Clip Out");
});
it("omits routine readiness badges while retaining actionable playback errors", () => {
	const view = render(<CueListClipStatus status={{ state: "armed", clip_id: "clip", lane_id: "lane", cue_list_id: "list", cue_id: null, cue_start_frame: null, level: 0, message: null }} />);
	expect(view.container).toBeEmptyDOMElement();
	view.rerender(<CueListClipStatus status={{ state: "unable", clip_id: "clip", lane_id: "lane", cue_list_id: "list", cue_id: null, cue_start_frame: null, level: 0, message: "Cuelist missing" }} />);
	expect(screen.getByRole("status")).toHaveTextContent("UnableCuelist missing");
});
