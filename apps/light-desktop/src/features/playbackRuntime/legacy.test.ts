import { describe, expect, it } from "vitest";
import { cueProjection } from "./testFixtures";
import { legacyPlaybackRuntime } from "./legacy";

describe("legacyPlaybackRuntime Cuelist timing", () => {
	it("preserves exact timing authority and transition identity", () => {
		const projection = cueProjection();
		if (projection.target !== "cue_list" || !projection.runtime)
			throw new Error("fixture must contain a running Cuelist");
		const cueTiming = {
			cue_id: projection.runtime.current!.id,
			in_delay_millis: 100,
			in_fade_millis: 900,
			out_delay_millis: 200,
			out_fade_millis: 1_800,
			completion_millis: 2_000,
			active_trigger: null,
			completed_trigger_cue_id: null,
		};
		projection.runtime.paused_at = "2026-08-08T12:00:00.500Z";
		projection.runtime.cue_timing = cueTiming;
		projection.runtime.transition_ordinal = 19;

		expect(legacyPlaybackRuntime(projection)).toEqual(
			expect.objectContaining({
				paused_at: "2026-08-08T12:00:00.500Z",
				cue_timing: cueTiming,
				transition_ordinal: 19,
			}),
		);
	});
});
