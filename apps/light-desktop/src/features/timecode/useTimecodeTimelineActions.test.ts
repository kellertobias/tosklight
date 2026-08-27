import { describe, expect, it } from "vitest";
import type { Cue, CueList } from "../../api/types";
import type { TimecodeCueListClip } from "../../api/types/timecode";
import { cueClipPlacement } from "./useTimecodeTimelineActions";

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
