import { cleanup, fireEvent, render, renderHook, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { CueList } from "../../api/types";
import type { TimecodeDefinition, TimecodeCueListClip } from "../../api/types/timecode";
import { CueClipBoundaryProvider, cueClipBoundarySlots } from "./CueClipBoundaryContext";
import { CueTimingInspector } from "./TimecodeCueTimingInspector";
import { SelectionProperties } from "./TimecodeSelectionInspector";
import { cueClipTimingRows } from "./cueClipTiming";
import { useCueClipBoundaryResize } from "./useCueClipBoundaryResize";
import { timelineItems } from "./editorModel";

afterEach(cleanup);
const clip: TimecodeCueListClip = {
	id: "clip", start_frame: 44, end_frame: 220, start_cue_id: "cue", end_cue_id: "cue",
	start_behavior: "state", end_behavior: "hold", cue_starts: [], in_fade_frames: 0, out_fade_frames: 0,
};
const definition: TimecodeDefinition = {
	id: "timecode", number: 1, name: "Song", duration_frame: 440,
	transport_offset_frame: 0, auto_start: false, markers: [],
	lanes: [{ id: "lane", name: "Opening", content: { kind: "cue_list", cue_list_id: "list", clips: [clip] } }],
};
const selection = { kind: "clip", laneId: "lane", itemId: "clip" } as const;
const list: CueList = { id: "list", name: "Opening", mode: "sequence", priority: 0, looped: false,
	cues: [{ id: "cue", number: "1", name: "First", fade_millis: 1000, delay_millis: 0, trigger: { type: "manual" }, changes: [] }] };
const defaults = { sequenceFadeMillis: 0, releaseFadeMillis: 0 };

it("places each clip boundary with its Cue timing section and delegates to the existing resize callback", () => {
	const resize = vi.fn();
	const commitTiming = vi.fn();
	const commitDefinition = vi.fn();
	const row = cueClipTimingRows(clip, list, defaults).rows[0];
	const view = render(<CueClipBoundaryProvider value={cueClipBoundarySlots({ definition, selection, onResizeClip: resize })}>
		<CueTimingInspector selected={row} clip={clip} canMoveTrigger placeTrigger={vi.fn()} source="Clip start" saving={false}
			onSaveCueList={vi.fn()} commit={commitTiming} timingDefaults={defaults} />
		<SelectionProperties definition={definition} selection={selection} cueTimingActive cueLists={[{ id: list.id, name: list.name, cues: list.cues, body: list }]}
			fps={44} items={timelineItems(definition)} onCommit={commitDefinition} onSelect={vi.fn()} />
	</CueClipBoundaryProvider>);
	const start = screen.getByLabelText("Start frame");
	const end = screen.getByLabelText("End frame");
	expect(start.closest("details")?.querySelector("summary")).toHaveTextContent("Cue In");
	expect(end.closest("details")?.querySelector("summary")).toHaveTextContent("Cue Out");
	expect(screen.getByLabelText("In start").closest("details")).toBe(start.closest("details"));
	expect(screen.getByLabelText("Out end").closest("details")).toBe(end.closest("details"));
	expect(view.container.querySelectorAll('details[open]')).toHaveLength(2);
	const cuelist = screen.getByText("Cuelist", { selector: "summary" }).parentElement!;
	expect(cuelist).not.toHaveAttribute("open");
	expect(screen.queryByRole("button", { name: "Delete clip" })).not.toBeInTheDocument();
	expect(screen.getByRole("button", { name: "Copy clip" })).toBeInTheDocument();
	fireEvent.change(start, { target: { value: "66" } });
	fireEvent.change(end, { target: { value: "264" } });
	expect(resize.mock.calls).toEqual([[selection, "start", 66], [selection, "end", 264]]);
	expect(commitDefinition).not.toHaveBeenCalled();
	expect(commitTiming).not.toHaveBeenCalled();
});

it("does not create a definition overwrite path when resize authority is unavailable", () => {
	const slots = cueClipBoundarySlots({ definition, selection });
	render(<>{slots?.inControl}{slots?.outControl}</>);
	expect(screen.getByLabelText("Start frame")).toBeDisabled();
	expect(screen.getByLabelText("End frame")).toBeDisabled();
});

it("exact boundary resize scales authoritative Cue timing within the same gesture and ignores unchanged frames", () => {
	const steps: string[] = [];
	const preview = vi.fn(() => { steps.push("preview"); });
	const save = vi.fn(async (_id: string, body: CueList) => { steps.push("save"); return body; });
	const begin = vi.fn(() => { steps.push("begin"); });
	const end = vi.fn(() => { steps.push("end"); });
	const { result } = renderHook(() => useCueClipBoundaryResize({
		definition, cueLists: [{ id: list.id, name: list.name, cues: list.cues, body: list }],
		onBeginGesture: begin, onPreview: preview, onEndGesture: end, onSaveCueList: save,
	}));
	result.current(selection, "end", 220);
	expect(steps).toEqual([]);
	result.current(selection, "end", 396);
	expect(steps).toEqual(["begin", "preview", "save", "end"]);
	expect(save).toHaveBeenCalledOnce();
	expect(save.mock.calls[0][1].cues[0].fade_millis).toBe(2000);
	expect(begin).toHaveBeenCalledOnce();
	expect(end).toHaveBeenCalledOnce();
});
