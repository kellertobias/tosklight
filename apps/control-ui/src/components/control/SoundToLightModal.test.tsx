import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { SoundToLightConfig, SpeedGroupSoundState } from "../../api/types";
import { inactiveCaptureStatus } from "./soundToLightAnalyzer";
import { SoundToLightModal } from "./SoundToLightModal";

const configuration: SoundToLightConfig = {
  enabled: false,
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

const state: SpeedGroupSoundState = {
  group: "A",
  configuration,
  snapshot: {
    manual_bpm: 120,
    sound_bpm: null,
    effective_bpm: 120,
    source: "manual",
    sound_status: { state: "disabled" },
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

afterEach(cleanup);

describe("SoundToLightModal", () => {
  it("configures a portable sound profile while keeping the input assignment browser-local", async () => {
    const onDeviceChange = vi.fn();
    const onPreview = vi.fn();
    const onSave = vi.fn(async (next: SoundToLightConfig) => ({ ...state, configuration: next }));
    render(<SoundToLightModal
      group="A"
      state={state}
      capture={{ ...inactiveCaptureStatus, observation: { captured_at_millis: 1, source_available: true, usable_signal: true, level: 0.4, selected_band_level: 0.7, detected_bpm: 128, confidence: 0.91 } }}
      permission="granted"
      devices={[{ deviceId: "line-in", label: "USB Line Input" }]}
      deviceId=""
      onDeviceChange={onDeviceChange}
      onRefreshInputs={vi.fn(async () => undefined)}
      onPreview={onPreview}
      onSave={onSave}
      onAction={vi.fn(async () => state)}
      onClose={vi.fn()}
    />);

    expect(screen.getByRole("dialog", { name: "Speed Group A Sound to Light" })).toBeInTheDocument();
    expect(screen.getByText("128.0 BPM")).toBeInTheDocument();
    expect(screen.getByRole("meter", { name: "Selected band" })).toHaveAttribute("aria-valuenow", "70");
    expect(screen.getByText(/device ID stays in this browser/i)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Not assigned on this browser" }));
    fireEvent.click(screen.getByRole("option", { name: "System default input" }));
    expect(onDeviceChange).toHaveBeenCalledWith("default");

    fireEvent.click(screen.getByRole("switch", { name: "Enable Sound-to-Light" }));
    fireEvent.click(screen.getByRole("button", { name: "Low · 60–180 Hz" }));
    fireEvent.click(screen.getByRole("option", { name: "Custom range" }));
    const lowControl = screen.getByLabelText("Custom low frequency").closest(".ui-number-control")!;
    fireEvent.click(within(lowControl as HTMLElement).getByRole("button", { name: "Decrease value" }));
    await waitFor(() => expect(onPreview).toHaveBeenCalledWith("A", expect.objectContaining({ frequency: { type: "custom", low_hz: 59, high_hz: 180 } })));
    fireEvent.click(within(lowControl as HTMLElement).getByRole("button", { name: "Open number pad" }));
    let numberPad = screen.getByRole("dialog", { name: "Low frequency" });
    fireEvent.click(within(numberPad).getByRole("button", { name: "←" }));
    fireEvent.click(within(numberPad).getByRole("button", { name: "←" }));
    fireEvent.click(within(numberPad).getByRole("button", { name: "4" }));
    fireEvent.click(within(numberPad).getByRole("button", { name: "5" }));
    fireEvent.click(within(numberPad).getByRole("button", { name: "ENTER" }));
    const highControl = screen.getByLabelText("Custom high frequency").closest(".ui-number-control")!;
    fireEvent.click(within(highControl as HTMLElement).getByRole("button", { name: "Open number pad" }));
    numberPad = screen.getByRole("dialog", { name: "High frequency" });
    fireEvent.click(within(numberPad).getByRole("button", { name: "←" }));
    fireEvent.click(within(numberPad).getByRole("button", { name: "←" }));
    fireEvent.click(within(numberPad).getByRole("button", { name: "←" }));
    fireEvent.click(within(numberPad).getByRole("button", { name: "1" }));
    fireEvent.click(within(numberPad).getByRole("button", { name: "4" }));
    fireEvent.click(within(numberPad).getByRole("button", { name: "0" }));
    fireEvent.click(within(numberPad).getByRole("button", { name: "ENTER" }));
    fireEvent.change(screen.getByLabelText("Input gain"), { target: { value: "6" } });
    fireEvent.change(screen.getByLabelText("Sound multiplier"), { target: { value: "2" } });
    fireEvent.click(screen.getByRole("button", { name: "Apply" }));

    await waitFor(() => expect(onSave).toHaveBeenCalledTimes(1));
    const saved = onSave.mock.calls[0][0];
    expect(saved).toMatchObject({
      enabled: true,
      frequency: { type: "custom", low_hz: 45, high_hz: 140 },
      input_gain_db: 6,
      multiplier: 2,
    });
    expect(saved).not.toHaveProperty("device_id");
    expect(saved).not.toHaveProperty("deviceId");
    expect(onPreview).toHaveBeenCalledWith("A", expect.objectContaining({ enabled: true }));
  });

  it("exposes Learn, ratio, and Pause through the authoritative action endpoint", async () => {
    const onAction = vi.fn(async () => state);
    render(<SoundToLightModal
      group="A"
      state={state}
      capture={inactiveCaptureStatus}
      permission="prompt"
      devices={[]}
      deviceId=""
      onDeviceChange={vi.fn()}
      onRefreshInputs={vi.fn(async () => undefined)}
      onPreview={vi.fn()}
      onSave={vi.fn(async () => state)}
      onAction={onAction}
      onClose={vi.fn()}
    />);
    fireEvent.click(screen.getByRole("button", { name: "Learn" }));
    await waitFor(() => expect(onAction).toHaveBeenCalledWith(expect.objectContaining({ action: "learn", captured_at_millis: expect.any(Number) })));
    fireEvent.click(screen.getByRole("button", { name: "Half" }));
    await waitFor(() => expect(onAction).toHaveBeenCalledWith({ action: "half" }));
    fireEvent.click(screen.getByRole("button", { name: "Double" }));
    await waitFor(() => expect(onAction).toHaveBeenCalledWith({ action: "double" }));
    fireEvent.click(screen.getByRole("button", { name: "Pause" }));
    await waitFor(() => expect(onAction).toHaveBeenCalledWith({ action: "pause" }));
  });
});
