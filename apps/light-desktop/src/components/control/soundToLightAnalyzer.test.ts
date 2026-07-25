import { describe, expect, it } from "vitest";
import type { SoundToLightConfig } from "../../api/types";
import { analyzeAudioFrame, frequencyRange, SoundTempoTracker } from "./soundToLightAnalyzer";

const configuration: SoundToLightConfig = {
  enabled: true,
  analysis_mode: "tempo_bpm",
  frequency: { type: "preset", preset: "low" },
  input_gain_db: 0,
  confidence_threshold: 0.65,
  smoothing: 0.35,
  minimum_bpm: 40,
  maximum_bpm: 240,
  signal_hold_millis: 2_000,
  multiplier: 1,
};

function frame(level: number) {
  const values = new Uint8Array(1_024);
  values.fill(2);
  for (let index = 3; index <= 8; index += 1) values[index] = Math.round(level * 255);
  return values;
}

describe("Sound-to-Light audio analysis", () => {
  it("maps the named frequency regions to the authoritative Rust ranges", () => {
    expect(frequencyRange({ type: "preset", preset: "sub" })).toEqual([30, 80]);
    expect(frequencyRange({ type: "preset", preset: "low" })).toEqual([60, 180]);
    expect(frequencyRange({ type: "preset", preset: "mid" })).toEqual([180, 2_000]);
    expect(frequencyRange({ type: "preset", preset: "high" })).toEqual([2_000, 12_000]);
    expect(frequencyRange({ type: "preset", preset: "full_range" })).toEqual([30, 18_000]);
    expect(frequencyRange({ type: "custom", low_hz: 75, high_hz: 135 })).toEqual([75, 135]);
  });

  it("derives a stable normalized 120 BPM observation from repeated low-band transients", () => {
    const tracker = new SoundTempoTracker();
    const timeDomain = new Float32Array(2_048).fill(0.05);
    let observation = analyzeAudioFrame(timeDomain, frame(0.02), 48_000, 2_048, configuration, tracker, 0);
    for (let capturedAt = 0; capturedAt <= 1_500; capturedAt += 100) {
      const transient = capturedAt % 500 === 0;
      observation = analyzeAudioFrame(timeDomain, frame(transient ? 0.9 : 0.02), 48_000, 2_048, configuration, tracker, capturedAt);
    }
    expect(observation.source_available).toBe(true);
    expect(observation.usable_signal).toBe(true);
    expect(observation.level).toBeGreaterThan(0);
    expect(observation.selected_band_level).toBeGreaterThan(0);
    expect(observation.detected_bpm).toBeCloseTo(120, 1);
    expect(observation.confidence).toBeGreaterThanOrEqual(0.65);
    expect(observation.level).toBeLessThanOrEqual(1);
    expect(observation.selected_band_level).toBeLessThanOrEqual(1);
  });

  it("reports quiet selected-band audio as unusable without inventing a tempo", () => {
    const observation = analyzeAudioFrame(
      new Float32Array(2_048).fill(0.0001),
      frame(0),
      48_000,
      2_048,
      configuration,
      new SoundTempoTracker(),
      100,
    );
    expect(observation.usable_signal).toBe(false);
    expect(observation.detected_bpm).toBeNull();
    expect(observation.confidence).toBe(0);
  });
});
