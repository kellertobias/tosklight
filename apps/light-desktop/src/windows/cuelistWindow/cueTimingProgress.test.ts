import { describe, expect, it } from "vitest";
import type { Cue, PlaybackSnapshot } from "../../api/types";
import { deriveCueTimingProgress } from "./cueTimingProgress";

const START = Date.parse("2026-08-08T12:00:00.000Z");
const cues: Cue[] = [
	{
		id: "source",
		number: 1,
		name: "Source",
		fade_millis: 1_000,
		delay_millis: 500,
		trigger: { type: "link", cue_id: "current", delay_millis: 400 },
		changes: [],
	},
	{
		id: "current",
		number: 2,
		name: "Current",
		fade_millis: 1_000,
		delay_millis: 500,
		trigger: { type: "manual" },
		changes: [],
	},
	{
		id: "target",
		number: 3,
		name: "Target",
		fade_millis: 0,
		delay_millis: 0,
		trigger: { type: "wait", delay_millis: 400 },
		changes: [],
	},
];

function runtime(
	overrides: Partial<PlaybackSnapshot["active"][number]> = {},
): PlaybackSnapshot["active"][number] {
	return {
		cue_list_id: "list",
		cue_index: 1,
		previous_index: 0,
		paused: false,
		activated_at: new Date(START).toISOString(),
		paused_at: null,
		transition_ordinal: 7,
		master: 1,
		flash: false,
		cue_timing: {
			cue_id: "current",
			in_delay_millis: 500,
			in_fade_millis: 1_000,
			out_delay_millis: 250,
			out_fade_millis: 500,
			completion_millis: 1_500,
			active_trigger: null,
			completed_trigger_cue_id: null,
		},
		...overrides,
	};
}

describe("Cuelist timing progress", () => {
	it("keeps incoming phases on the new Cue and outgoing phases on the previous Cue", () => {
		expect(deriveCueTimingProgress(cues, runtime(), START)[1]).toEqual({
			inDelay: 0,
			inFade: 0,
		});
		expect(deriveCueTimingProgress(cues, runtime(), START)[0]).toEqual({ outDelay: 0, outFade: 0 });
		expect(deriveCueTimingProgress(cues, runtime(), START + 750)[1]).toEqual({
			inDelay: 1,
			inFade: 0.25,
		});
		expect(deriveCueTimingProgress(cues, runtime(), START + 750)[0]).toEqual({ outDelay: 1, outFade: 1 });
		expect(deriveCueTimingProgress(cues, runtime(), START + 2_000)[1]).toEqual({
			inDelay: 1,
			inFade: 1,
		});
		expect(deriveCueTimingProgress(cues, runtime(), START + 2_000)[0]).toEqual({ outDelay: 1, outFade: 1 });
	});

	it("treats zero-duration phases as complete once activation starts", () => {
		const zero = runtime({
			cue_timing: {
				...runtime().cue_timing!,
				in_delay_millis: 0,
				in_fade_millis: 0,
				out_delay_millis: 0,
				out_fade_millis: 0,
			},
		});
		expect(deriveCueTimingProgress(cues, zero, START)[1]).toEqual({
			inDelay: 1,
			inFade: 1,
		});
		expect(deriveCueTimingProgress(cues, zero, START)[0]).toEqual({ outDelay: 1, outFade: 1 });
	});

	it("freezes every phase at paused_at instead of the advancing wall clock", () => {
		const paused = runtime({
			paused: true,
			paused_at: new Date(START + 750).toISOString(),
		});
		expect(deriveCueTimingProgress(cues, paused, START + 10_000)[1]?.inFade).toBe(0.25);
	});

	it("starts a looping TIME trigger for Cue 1 when Cue 2 receives GO", () => {
		const loopingCues: Cue[] = [
			{ ...cues[0], trigger: { type: "wait", delay_millis: 400 } },
			{ ...cues[1], trigger: { type: "manual" } },
		];
		const looping = runtime({
			cue_index: 1,
			cue_timing: {
				...runtime().cue_timing!,
				cue_id: "current",
				active_trigger: {
					cue: { id: "source", number: 1 },
					kind: "wait",
					started_at: new Date(START).toISOString(),
					duration_millis: 400,
				},
			},
		});
		expect(deriveCueTimingProgress(loopingCues, looping, START)[0]?.triggerTime).toBe(0);
		expect(deriveCueTimingProgress(loopingCues, looping, START + 200)[0]?.triggerTime).toBe(0.5);
	});

	it("associates Link timing with its source row", () => {
		const link = runtime({
			cue_timing: {
				...runtime().cue_timing!,
				active_trigger: {
					cue: { id: "source", number: 1 },
					kind: "link",
					started_at: new Date(START).toISOString(),
					duration_millis: 400,
				},
			},
		});
		expect(deriveCueTimingProgress(cues, link, START + 100)[0]?.triggerTime).toBe(0.25);
	});

	it("latches completed trigger progress and resets on a new activation", () => {
		const completed = runtime({
			cue_timing: {
				...runtime().cue_timing!,
				completed_trigger_cue_id: "target",
			},
		});
		expect(deriveCueTimingProgress(cues, completed, START)[2]?.triggerTime).toBe(1);

		const restarted = runtime({
			activated_at: new Date(START + 5_000).toISOString(),
			transition_ordinal: 8,
			cue_timing: {
				...runtime().cue_timing!,
				completed_trigger_cue_id: null,
			},
		});
		expect(deriveCueTimingProgress(cues, restarted, START + 5_000)[1]?.inFade).toBe(0);
		expect(deriveCueTimingProgress(cues, restarted, START + 5_000)[2]?.triggerTime).toBeUndefined();
	});
});
