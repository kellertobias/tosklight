// @vitest-environment jsdom
import { act, fireEvent, renderHook } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { Cue, CueList } from "../../api/types";
import type {
	TimecodeCueListClip,
	TimecodeDefinition,
} from "../../api/types/timecode";
import {
	cueClipPlacement,
	useTimelineDrag,
} from "./useTimecodeTimelineActions";

const timingDefaults = { sequenceFadeMillis: 3_000, releaseFadeMillis: 3_000 };

describe("new Cue List clip placement", () => {
	it("takes the rest of the lane from the playhead on an empty lane", () => {
		expect(
			cueClipPlacement({
				clips: [],
				cueList: option(),
				frame: 100,
				duration: 440,
				timingDefaults,
			}),
		).toEqual({
			start_frame: 100,
			end_frame: 440,
			start_cue_id: "cue-1",
			end_cue_id: "cue-3",
			start_behavior: "state",
			end_behavior: "release",
			cue_starts: [],
			in_fade_frames: 0,
			out_fade_frames: 0,
		});
	});

	it("copies length and configuration from the last clip left of the playhead", () => {
		const placement = cueClipPlacement({
			clips: [
				clip({ id: "early", start_frame: 0, end_frame: 44 }),
				clip({
					id: "previous",
					start_frame: 88,
					end_frame: 176,
					start_cue_id: "cue-2",
					end_cue_id: "cue-2",
					end_behavior: "hold",
				}),
				clip({ id: "later", start_frame: 300, end_frame: 400 }),
			],
			cueList: option(),
			frame: 200,
			duration: 440,
			timingDefaults,
		});
		expect(placement).toEqual({
			start_frame: 200,
			end_frame: 288,
			start_cue_id: "cue-2",
			end_cue_id: "cue-2",
			start_behavior: "state",
			end_behavior: "hold",
			cue_starts: [],
			in_fade_frames: 0,
			out_fade_frames: 0,
		});
	});

	it("takes its length from a fully automated Cue List", () => {
		const placement = cueClipPlacement({
			clips: [],
			cueList: {
				...option(),
				body: cueList([
					cue("cue-1", "1", { type: "follow", delay_millis: 0 }),
					cue("cue-2", "2", { type: "follow", delay_millis: 0 }),
					cue("cue-3", "3", { type: "follow", delay_millis: 0 }),
				]),
			},
			frame: 44,
			duration: 440,
			timingDefaults,
		});
		expect(placement?.start_frame).toBe(44);
		// Three chained one-second Cues complete 132 frames after the clip start.
		expect(placement?.end_frame).toBe(176);
	});

	it("keeps the copied length when the Cue List needs a manual GO", () => {
		const placement = cueClipPlacement({
			clips: [clip({ id: "previous", start_frame: 0, end_frame: 88 })],
			cueList: {
				...option(),
				body: cueList([
					cue("cue-1", "1", { type: "follow", delay_millis: 0 }),
					cue("cue-2", "2", { type: "manual" }),
					cue("cue-3", "3", { type: "follow", delay_millis: 0 }),
				]),
			},
			frame: 132,
			duration: 440,
			timingDefaults,
		});
		expect(placement).toMatchObject({ start_frame: 132, end_frame: 220 });
	});

	it("refuses to place a clip for a Cue List without stable Cue identities", () => {
		expect(
			cueClipPlacement({
				clips: [],
				cueList: { id: "list", name: "Opening", cues: [] },
				frame: 0,
				duration: 440,
				timingDefaults,
			}),
		).toBeNull();
	});
});

function clip(patch: Partial<TimecodeCueListClip>): TimecodeCueListClip {
	return {
		id: "clip",
		start_frame: 0,
		end_frame: 44,
		start_cue_id: "cue-1",
		end_cue_id: "cue-3",
		start_behavior: "state",
		end_behavior: "release",
		in_fade_frames: 0,
		out_fade_frames: 0,
		cue_starts: [],
		...patch,
	};
}

function option() {
	return {
		id: "list",
		name: "Opening",
		cues: [
			{ id: "cue-1", number: "1", name: "Cue 1" },
			{ id: "cue-2", number: "2", name: "Cue 2" },
			{ id: "cue-3", number: "3", name: "Cue 3" },
		],
	};
}

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

it("uses the current Cue timing callback when a resize ends after a render", () => {
	let current: TimecodeDefinition = {
		id: "tc-current",
		number: 1,
		name: "Current",
		duration_frame: 2640,
		transport_offset_frame: 0,
		auto_start: false,
		markers: [],
		lanes: [
			{
				id: "lane",
				name: "Cue",
				content: {
					kind: "cue_list",
					cue_list_id: "list",
					clips: [
						{
							id: "clip",
							start_frame: 0,
							end_frame: 264,
							start_cue_id: "cue-1",
							end_cue_id: "cue-1",
							start_behavior: "state",
							end_behavior: "release",
							cue_starts: [],
							in_fade_frames: 0,
							out_fade_frames: 0,
						},
					],
				},
			},
		],
	};
	const original = vi.fn();
	const latest = vi.fn();
	const preview = vi.fn((next: TimecodeDefinition) => {
		current = next;
	});
	const begin = vi.fn();
	const end = vi.fn();
	const select = vi.fn();
	const hook = renderHook(
		({ definition, callback }) =>
			useTimelineDrag({
				definition,
				pixelsPerFrame: 1,
				onPreview: preview,
				onBeginGesture: begin,
				onEndGesture: end,
				setSelection: select,
				onScaleCueTimings: callback,
			}),
		{ initialProps: { definition: current, callback: original } },
	);
	act(() => {
		hook.result.current(
			{
				preventDefault: vi.fn(),
				clientX: 0,
				clientY: 0,
				currentTarget: { setPointerCapture: vi.fn() },
			} as never,
			{ kind: "clip", laneId: "lane", itemId: "clip" },
			0,
			"start",
		);
	});
	fireEvent.pointerMove(window, { clientX: 154 });
	hook.rerender({ definition: current, callback: latest });
	fireEvent.pointerUp(window);
	expect(original).not.toHaveBeenCalled();
	expect(latest).toHaveBeenCalledWith(
		{ kind: "clip", laneId: "lane", itemId: "clip" },
		110 / 264,
	);
});


it("volume drags follow the actual lane value axis at zero, middle, and full level", () => {
	for (const height of [100, 200]) {
		for (const value of [0, 0.5, 1]) {
			const lane = document.createElement("div");
			lane.className = "timecode-editor-lane";
			Object.defineProperty(lane, "clientHeight", { value: height });
			const button = document.createElement("button");
			lane.append(button);
			const definition: TimecodeDefinition = {
				id: "volume", number: 1, name: "Audio", duration_frame: 440,
				transport_offset_frame: 0, auto_start: false, markers: [],
				lanes: [{ id: "audio", name: "Audio", content: { kind: "audio_volume", keyframes: [
					{ id: "point", frame: 220, value: 0.5, fade_frames: 0, curve: "linear" },
				] } }],
			};
			const preview = vi.fn();
			const hook = renderHook(() => useTimelineDrag({
				definition, pixelsPerFrame: 2, onPreview: preview,
				onBeginGesture: vi.fn(), onEndGesture: vi.fn(), setSelection: vi.fn(),
			}));
			act(() => hook.result.current({ preventDefault: vi.fn(), clientX: 100, clientY: height / 2,
				currentTarget: button } as never, { kind: "volume", laneId: "audio", itemId: "point" }, 220, undefined, 0.5));
			fireEvent.pointerMove(window, { clientX: 100, clientY: (88 - value * 76) / 100 * height });
			const next = preview.mock.calls.at(-1)![0] as TimecodeDefinition;
			const content = next.lanes[0].content;
			if (content.kind !== "audio_volume") throw Error("Audio lane expected");
			expect(content.keyframes[0].value).toBeCloseTo(value, 8);
			expect(content.keyframes[0].frame).toBe(220);
			fireEvent.pointerUp(window);
			hook.unmount();
		}
	}
});
