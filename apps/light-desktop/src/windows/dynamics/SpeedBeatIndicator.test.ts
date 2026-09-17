import { describe, expect, it } from "vitest";
import type { DynamicRuntimeSnapshotProjection } from "../../api/types";
import {
	BEAT_SAMPLE_STALE_MILLIS,
	beatPhaseAt,
	type SpeedBeatSample,
	speedBeatSample,
	speedBeatState,
	stampRuntimeReceipt,
} from "./SpeedBeatIndicator";

const sample = (patch: Partial<SpeedBeatSample> = {}): SpeedBeatSample => ({
	group: "A",
	phase: 0.9,
	bpm: 120,
	advancing: true,
	receivedAt: 1_000,
	...patch,
});

function runtime(
	speedGroups: DynamicRuntimeSnapshotProjection["speed_groups"],
): DynamicRuntimeSnapshotProjection {
	return {
		global_paused: false,
		instances: [],
		definitions: [],
		speed_groups: speedGroups,
	};
}

describe("Speed Group beat timing", () => {
	it("advances the sampled phase at the sampled tempo and wraps at the beat", () => {
		// 120 BPM is one beat per 500 ms: 100 ms after 0.9 is 0.1 of the next beat.
		expect(beatPhaseAt(sample(), 1_100)).toBeCloseTo(0.1);
		expect(speedBeatState(sample(), 1_100)).toBe("flash");
		expect(speedBeatState(sample(), 1_000)).toBe("dark");
		expect(speedBeatState(sample(), 1_200)).toBe("dark");
		expect(speedBeatState(sample(), 1_560)).toBe("flash");
	});

	it("follows a tempo change from the next sample instead of the old rate", () => {
		const slow = sample({ bpm: 60, phase: 0, receivedAt: 0 });
		const fast = sample({ bpm: 240, phase: 0, receivedAt: 0 });
		expect(speedBeatState(slow, 300)).toBe("dark");
		expect(beatPhaseAt(fast, 300)).toBeCloseTo(0.2);
		expect(speedBeatState(fast, 250)).toBe("flash");
	});

	it("holds a paused group on its frozen phase without flashing", () => {
		const paused = sample({ advancing: false, phase: 0.05 });
		expect(beatPhaseAt(paused, 60_000)).toBe(0.05);
		expect(speedBeatState(paused, 1_400)).toBe("paused");
	});

	it("stops claiming a beat while the desk is not answering and recovers on the next sample", () => {
		const last = sample();
		expect(speedBeatState(null, 0)).toBe("unavailable");
		expect(speedBeatState(last, 1_000 + BEAT_SAMPLE_STALE_MILLIS + 1)).toBe(
			"unavailable",
		);
		const reconnected = sample({ phase: 0, receivedAt: 9_000 });
		expect(speedBeatState(reconnected, 9_010)).toBe("flash");
	});

	it("reads the addressed group from a stamped runtime snapshot", () => {
		const snapshot = stampRuntimeReceipt(
			runtime([
				{
					group: "A",
					effective_bpm: 120,
					beat_phase: 0.5,
					phase_advancing: true,
					paused: false,
				},
				{
					group: "C",
					effective_bpm: 90,
					beat_phase: 0.25,
					phase_advancing: false,
					paused: true,
				},
			]),
			4_200,
		);
		expect(speedBeatSample(snapshot, "C")).toEqual({
			group: "C",
			phase: 0.25,
			bpm: 90,
			advancing: false,
			receivedAt: 4_200,
		});
		expect(speedBeatSample(snapshot, "B")).toBeNull();
		expect(speedBeatSample(null, "A")).toBeNull();
	});
});
