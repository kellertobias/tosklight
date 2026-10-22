import { cleanup, fireEvent, render, renderHook, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import type { TimecodeDefinition } from "../../api/types/timecode";
import { timelineItems, type TimecodeEditorSelection } from "./editorModel";
import { TimelineTitleStateContext, useTimelineTitleActions, EMPTY_TIMELINE_TITLE_STATE } from "./useTimelineTitleActions";
import { SelectionProperties } from "./TimecodeSelectionInspector";
import { timecodeTimelineTitleActions } from "./timecodeTimelineTitleActions";
import type { TimecodeTimelineEditorHandle } from "./TimecodeTimelineEditor";

afterEach(cleanup);
const definition: TimecodeDefinition = { id: "show-time", number: 1, name: "Song", duration_frame: 440, transport_offset_frame: 0, auto_start: false,
	markers: [{ id: "mark", frame: 22, name: "Marker" }], lanes: [
		{ id: "a", name: "A", content: { kind: "speed_group", group: "A", keyframes: [{ id: "a2", frame: 88, bpm: 130, phase: 0 }, { id: "a1", frame: 0, bpm: 120, phase: 0 }] } },
		{ id: "b", name: "B", content: { kind: "speed_group", group: "B", keyframes: [{ id: "b1", frame: 44, bpm: 140, phase: 0 }] } },
		{ id: "c", name: "Cuelist", content: { kind: "cue_list", cue_list_id: "list", clips: [{ id: "clip", start_frame: 44, end_frame: 220, start_cue_id: "one", end_cue_id: "one", start_behavior: "state", end_behavior: "hold", cue_starts: [], in_fade_frames: 0, out_fade_frames: 0 }] } },
	] };
it("steps frame order only within the chosen lane and publishes title state through context", () => {
	const select = vi.fn(); const scrub = vi.fn(); const commit = vi.fn(); const publish = vi.fn();
	const { result, rerender } = renderHook(({ selection }: { selection: TimecodeEditorSelection | null }) => useTimelineTitleActions({ definition, selection, laneId: "a", items: timelineItems(definition), onSelect: select, onScrub: scrub, onCommit: commit }), {
		initialProps: { selection: null as TimecodeEditorSelection | null }, wrapper: ({ children }) => <TimelineTitleStateContext.Provider value={publish}>{children}</TimelineTitleStateContext.Provider>,
	});
	expect(publish).toHaveBeenLastCalledWith({ canStep: true, canDeleteClip: false, canInsert: true, canDeleteKeyframe: false, canAddClip: false });
	result.current.stepKeyframe(1);
	expect(select).toHaveBeenLastCalledWith({ kind: "speed", laneId: "a", itemId: "a1" });
	expect(scrub).toHaveBeenLastCalledWith(0);
	rerender({ selection: { kind: "speed", laneId: "a", itemId: "a1" } });
	result.current.stepKeyframe(1);
	expect(select).toHaveBeenLastCalledWith({ kind: "speed", laneId: "a", itemId: "a2" });
	expect(scrub).toHaveBeenLastCalledWith(88);
	expect(commit).not.toHaveBeenCalled();
});
it("deletes only the actual selected clip and refuses stale selection", () => {
	const select = vi.fn(); const commit = vi.fn(); const publish = vi.fn();
	const chosen = { kind: "clip", laneId: "c", itemId: "clip" } as const;
	const { result, rerender } = renderHook(({ items }) => useTimelineTitleActions({ definition, selection: chosen, laneId: "c", items, onSelect: select, onCommit: commit, onStateChange: publish }), { initialProps: { items: timelineItems(definition) } });
	result.current.deleteSelection();
	expect(commit.mock.calls[0][0].lanes[2].content.clips).toEqual([]);
	expect(commit.mock.calls[0][0].lanes.slice(0, 2)).toEqual(definition.lanes.slice(0, 2));
	expect(select).toHaveBeenCalledWith(null);
	rerender({ items: [] }); commit.mockClear(); select.mockClear();
	expect(publish).toHaveBeenLastCalledWith({ canStep: false, canDeleteClip: false, canInsert: false, canDeleteKeyframe: false, canAddClip: true });
	result.current.deleteSelection();
	expect(commit).not.toHaveBeenCalled(); expect(select).not.toHaveBeenCalled();
});
it("title actions call the current editor handle and obey capability visibility", () => {
	const handle: TimecodeTimelineEditorHandle = { stepKeyframe: vi.fn(), deleteSelection: vi.fn(), insertKeyframe: vi.fn(), addClip: vi.fn(), fit: vi.fn(), addMarker: vi.fn(), chooseSpeedLane: vi.fn(), chooseCueListLane: vi.fn(), addAudioPlayerLane: vi.fn() };
	const ref = { current: handle };
	const disabled = timecodeTimelineTitleActions(ref, EMPTY_TIMELINE_TITLE_STATE);
	expect(disabled.filter((action) => action.disabled).map((action) => action.id)).toEqual(["previous-keyframe", "next-keyframe"]);
	expect(disabled.map((action) => action.id)).toEqual(["previous-keyframe", "next-keyframe", "fit-timeline"]);
	const actions = timecodeTimelineTitleActions(ref, { canStep: true, canDeleteClip: true, canInsert: true, canDeleteKeyframe: false, canAddClip: true });
	for (const action of actions) action.onPress?.(document.createElement("button"));
	expect(handle.stepKeyframe).toHaveBeenCalledWith(-1); expect(handle.stepKeyframe).toHaveBeenCalledWith(1);
	expect(handle.fit).toHaveBeenCalledOnce(); expect(handle.insertKeyframe).toHaveBeenCalledOnce(); expect(handle.addClip).toHaveBeenCalledOnce(); expect(handle.deleteSelection).toHaveBeenCalledOnce();
});

it("keeps Add clip available on an empty Audio Player lane without enabling Delete clip", () => {
	const empty: TimecodeDefinition = { ...definition, lanes: [{ id: "audio", name: "Player", content: { kind: "audio_player", fixture_id: "fixture", clips: [] } }] };
	const publish = vi.fn();
	renderHook(() => useTimelineTitleActions({ definition: empty, selection: null, laneId: "audio", items: timelineItems(empty), onSelect: vi.fn(), onCommit: vi.fn(), onStateChange: publish }));
	const state = publish.mock.calls.at(-1)![0];
	expect(state.canAddClip).toBe(true); expect(state.canDeleteClip).toBe(false);
	const actions = timecodeTimelineTitleActions({ current: null }, state);
	expect(actions.some((action) => action.id === "add-clip")).toBe(true);
	expect(actions.some((action) => action.id === "delete-clip")).toBe(false);
});

it("keeps touch BPM editing in the sidebar after removing the bottom strip", () => {
	const commit = vi.fn();
	render(<SelectionProperties definition={definition} selection={{ kind: "speed", laneId: "a", itemId: "a1" }} cueLists={[]} fps={44}
		items={timelineItems(definition)} onCommit={commit} onSelect={vi.fn()} />);
	fireEvent.change(screen.getByRole("textbox", { name: "BPM value" }), { target: { value: "155" } });
	const saved = commit.mock.calls.at(-1)![0] as TimecodeDefinition;
	const lane = saved.lanes[0]; if (lane.content.kind !== "speed_group") throw new Error("speed lane expected");
	expect(lane.content.keyframes.find((item) => item.id === "a1")?.bpm).toBe(155);
	expect(saved.lanes[1]).toEqual(definition.lanes[1]);
	expect(screen.getByLabelText("Phase")).toBeInTheDocument();
});
it("keeps touch Volume, easing and fade fields in the sidebar", () => {
	const volume: TimecodeDefinition = { ...definition, lanes: [{ id: "volume", name: "Audio", content: { kind: "audio_volume", keyframes: [{ id: "point", frame: 44, value: 1, curve: "linear", fade_frames: 0 }] } }] };
	const commit = vi.fn();
	render(<SelectionProperties definition={volume} selection={{ kind: "volume", laneId: "volume", itemId: "point" }} cueLists={[]} fps={44}
		items={timelineItems(volume)} onCommit={commit} onSelect={vi.fn()} />);
	fireEvent.change(screen.getByRole("slider", { name: "Volume value" }), { target: { value: "25" } });
	expect(commit.mock.calls.at(-1)![0].lanes[0].content.keyframes[0].value).toBe(0.25);
	expect(screen.getByRole("button", { name: "Easing" })).toBeInTheDocument();
	expect(screen.getByLabelText("Fade frames")).toBeInTheDocument();
});
it("keeps marker name, color, placing and exact movement controls in the sidebar", () => {
	const commit = vi.fn();
	render(<SelectionProperties definition={definition} selection={{ kind: "marker", itemId: "mark" }} frame={88} cueLists={[]} fps={44}
		items={timelineItems(definition)} onCommit={commit} onSelect={vi.fn()} />);
	expect(screen.getByRole("button", { name: "Set Name" })).toBeInTheDocument();
	expect(screen.getByRole("button", { name: "Move To" })).toBeInTheDocument();
	fireEvent.click(screen.getByRole("button", { name: "Place Marker" }));
	expect(commit.mock.calls.at(-1)![0].markers[0]).toMatchObject({ id: "mark", name: "Marker", frame: 88 });
	fireEvent.click(screen.getByRole("button", { name: /Marker color: .*Select next color/ }));
	expect(commit.mock.calls.at(-1)![0].markers[0]).toMatchObject({ id: "mark", name: "Marker", frame: 22 });
	expect(commit.mock.calls.at(-1)![0].markers[0].color).toBeTruthy();
});
