import { describe, expect, it } from "vitest";
import type { SpeedGroupSoundState } from "../../api/types";
import { shouldPublishSoundObservation, soundDeviceStorageKey } from "./useSoundToLight";

function state(enabled: boolean): SpeedGroupSoundState {
  return {
    group: "A",
    configuration: {
      enabled,
      analysis_mode: "tempo_bpm",
      frequency: { type: "preset", preset: "low" },
      input_gain_db: 0,
      confidence_threshold: 0.65,
      smoothing: 0.35,
      minimum_bpm: 40,
      maximum_bpm: 240,
      signal_hold_millis: 2_000,
      multiplier: 1,
    },
    snapshot: {
      manual_bpm: 120,
      sound_bpm: null,
      effective_bpm: 120,
      source: "manual",
      sound_status: enabled ? { state: "manual_fallback", reason: "waiting_for_analysis" } : { state: "disabled" },
      paused: false,
      phase_advancing: true,
      speed_master_scale: 1,
      sound_multiplier: 1,
      source_available: false,
      usable_signal: false,
      input_level: 0,
      selected_band_level: 0,
    },
  };
}

describe("Sound-to-Light capture ownership", () => {
  it("keeps preview observations local until enabled show state has been applied", () => {
    expect(shouldPublishSoundObservation(undefined)).toBe(false);
    expect(shouldPublishSoundObservation(state(false))).toBe(false);
    expect(shouldPublishSoundObservation(state(true))).toBe(true);
  });

  it("scopes machine-specific device IDs by desk and Speed Group", () => {
    expect(soundDeviceStorageKey("desk-one", "A")).not.toBe(soundDeviceStorageKey("desk-two", "A"));
    expect(soundDeviceStorageKey("desk-one", "A")).not.toBe(soundDeviceStorageKey("desk-one", "B"));
  });
});
