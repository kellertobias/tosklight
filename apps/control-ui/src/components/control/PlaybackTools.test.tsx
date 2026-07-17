import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { SpeedGroupId, SpeedGroupSoundState } from "../../api/types";
import { PlaybackTools } from "./PlaybackTools";

const dispatch = vi.fn();
const state = {
  playbackPage: 0,
  playbackPageNames: ["Main", "Effects"],
  playbackSetArmed: false,
  shiftArmed: false,
};
const server = {
  session: null as { session_id: string; desk: { id: string } } | null,
  configuration: {
    programmer_fade_millis: 3_000,
    sequence_master_fade_millis: 4_000,
    speed_groups_bpm: [120, 90, 60, 30, 15],
  },
  playbacks: { active_page: 1, pages: [{ number: 1, name: "Main", slots: {} as Record<string, number> }] },
  setControlTiming: vi.fn(),
  setPlaybackPage: vi.fn(),
  savePlaybackPage: vi.fn(async () => true),
  speedGroup: vi.fn(),
  updateSpeedGroup: vi.fn(),
  observeSpeedGroup: vi.fn(),
  speedGroupAction: vi.fn(),
  commandLine: "FIXTURE",
  commandLinePristine: true,
  commandTargetMode: "FIXTURE" as const,
  setCommandLine: vi.fn(),
  executeCommandLine: vi.fn(),
};

vi.mock("../../state/AppContext", () => ({ useApp: () => ({ state, dispatch: (action: { type: string; value?: boolean }) => {
  if (action.type === "SET_PLAYBACK_SET_ARMED") state.playbackSetArmed = Boolean(action.value);
  if (action.type === "SET_SHIFT_ARMED") state.shiftArmed = Boolean(action.value);
  dispatch(action);
} }) }));
vi.mock("../../api/ServerContext", () => ({ useServer: () => server }));

afterEach(() => { cleanup(); server.session = null; server.playbacks = { active_page: 1, pages: [{ number: 1, name: "Main", slots: {} }] }; state.playbackPage = 0; state.playbackSetArmed = false; state.shiftArmed = false; server.commandLine = "FIXTURE"; server.commandLinePristine = true; if (typeof localStorage.clear === "function") localStorage.clear(); vi.clearAllMocks(); });

function soundState(group: SpeedGroupId): SpeedGroupSoundState {
  const bpm = server.configuration.speed_groups_bpm[group.charCodeAt(0) - 65];
  return {
    group,
    configuration: { enabled: false, analysis_mode: "tempo_bpm", frequency: { type: "preset", preset: "low" }, input_gain_db: 0, confidence_threshold: 0.65, smoothing: 0.35, minimum_bpm: 40, maximum_bpm: 240, signal_hold_millis: 2_000, multiplier: 1 },
    snapshot: { manual_bpm: bpm, sound_bpm: null, effective_bpm: bpm, source: "manual", sound_status: { state: "disabled" }, paused: false, phase_advancing: true, speed_master_scale: 1, sound_multiplier: 1, source_available: false, usable_signal: false, input_level: 0, selected_band_level: 0 },
  };
}

describe("PlaybackTools", () => {
  it("orders page controls, fade masters, and speed groups with icon-only chevrons", () => {
    const { container } = render(<PlaybackTools/>);
    const tools = container.querySelector(".playback-tools")!;
    expect([...tools.children].map((child) => child.className)).toEqual([
      "playback-command-keys",
      "playback-page-controls",
      "programmer-fade-fader full",
      "cue-fade-master",
      "speed-group-stack",
    ]);
    expect(within(container.querySelector(".playback-command-keys")!).getAllByRole("button").map((button) => button.textContent)).toEqual(["SET", "CPY", "MOV", "DEL", "SHIFT"]);
    const previous = screen.getByRole("button", { name: "Previous playback page" });
    const next = screen.getByRole("button", { name: "Next playback page" });
    expect(previous.textContent).toBe("");
    expect(next.textContent).toBe("");
    expect(previous.querySelector("svg path")).toBeInTheDocument();
    expect(next.querySelector("svg path")).toBeInTheDocument();
    const current = screen.getByRole("button", { name: "Select playback page. Page 1 Main" });
    expect(within(current).getByText("Page")).toBeInTheDocument();
    expect(within(current).getByText("1")).toBeInTheDocument();
    expect(within(current).getByText("Main")).toBeInTheDocument();
    const speedGroupA = within(container.querySelector(".speed-group-stack")!).getByRole("button", { name: "Speed group A, 120 BPM" });
    expect([...speedGroupA.children].map((child) => child.className)).toEqual([
      "speed-group-label",
      "speed-group-value",
      "speed-group-unit",
    ]);
  });

  it("routes playback command keys through the shared command line behavior", () => {
    const { rerender } = render(<PlaybackTools/>);
    fireEvent.click(screen.getByRole("button", { name: "CPY" }));
    expect(server.setCommandLine).toHaveBeenCalledWith("COPY", false);

    fireEvent.click(screen.getByRole("button", { name: "SHIFT" }));
    expect(dispatch).toHaveBeenCalledWith({ type: "SET_SHIFT_ARMED", value: true });
    rerender(<PlaybackTools/>);
    expect(screen.getByRole("button", { name: "SHIFT" })).toHaveClass("active");

    fireEvent.click(screen.getByRole("button", { name: "DEL" }));
    expect(dispatch).toHaveBeenCalledWith({ type: "SET_MODAL", modal: "systemControlsOpen", value: true });
    expect(server.setCommandLine).toHaveBeenCalledTimes(1);
  });

  it("creates and selects the next page when the last page has an assignment", async () => {
    server.playbacks.pages[0].slots = { "1": 12 };
    render(<PlaybackTools/>);
    fireEvent.click(screen.getByRole("button", { name: "Next playback page" }));
    await waitFor(() => expect(server.savePlaybackPage).toHaveBeenCalledWith({ number: 2, name: "Page 2", slots: {} }));
    expect(server.setPlaybackPage).toHaveBeenCalledWith(2);
    expect(dispatch).toHaveBeenCalledWith({ type: "SET_PLAYBACK_PAGE", page: 1 });
  });

  it("keeps Next disabled on an empty last page but lets the page menu add one", async () => {
    render(<PlaybackTools/>);
    expect(screen.getByRole("button", { name: "Next playback page" })).toBeDisabled();
    fireEvent.click(screen.getByRole("button", { name: "Select playback page. Page 1 Main" }));
    const dialog = screen.getByRole("dialog", { name: "Playback pages" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Add new page" }));
    await waitFor(() => expect(server.savePlaybackPage).toHaveBeenCalledWith({ number: 2, name: "Page 2", slots: {} }));
    expect(server.setPlaybackPage).toHaveBeenCalledWith(2);
  });

  it("opens page rename with SET then Page and persists the trimmed name", async () => {
    render(<PlaybackTools/>);
    fireEvent.click(screen.getByRole("button", { name: "SET" }));
    fireEvent.click(screen.getByRole("button", { name: "Select playback page. Page 1 Main" }));
    const dialog = screen.getByRole("dialog", { name: "Rename playback page 1" });
    expect(dispatch).toHaveBeenCalledWith({ type: "SET_PLAYBACK_SET_ARMED", value: false });
    fireEvent.change(within(dialog).getByRole("textbox", { name: "Playback page name" }), { target: { value: "  Act One  " } });
    fireEvent.click(within(dialog).getByRole("button", { name: "Rename Page" }));
    await waitFor(() => expect(server.savePlaybackPage).toHaveBeenCalledWith({ number: 1, name: "Act One", slots: {} }));
  });

  it("opens the selected Speed Group Sound-to-Light configuration instead of treating the UI button as a Learn tap", async () => {
    server.session = { session_id: "session-a", desk: { id: "desk-a" } };
    server.speedGroup.mockImplementation(async (group: SpeedGroupId) => soundState(group));
    render(<PlaybackTools/>);
    await waitFor(() => expect(server.speedGroup).toHaveBeenCalledTimes(5));
    fireEvent.click(screen.getByRole("button", { name: "Speed group A, 120 BPM" }));
    expect(await screen.findByRole("dialog", { name: "Speed Group A Sound to Light" })).toBeInTheDocument();
    expect(screen.getByText("Audio input on this desk/browser")).toBeInTheDocument();
    expect(server.setControlTiming).not.toHaveBeenCalled();
    expect(server.speedGroupAction).not.toHaveBeenCalled();
  });
});
