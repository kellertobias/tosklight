import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { PlaybackFaderBank, playbackRowUnits } from "./PlaybackFaderBank";
import { UPDATE_TARGET_EVENT } from "./updateWorkflow";

describe("playback row height units", () => {
  const oneButton = { first_playback_slot: 1, has_fader: false, button_count: 1 };
  const multipleButtons = { first_playback_slot: 11, has_fader: false, button_count: 3 };
  const fader = { first_playback_slot: 21, has_fader: true, button_count: 1 };

  it("uses 1/1/2 units with attached hardware", () => {
    expect([oneButton, multipleButtons, fader].map((row) => playbackRowUnits(row, true))).toEqual([1, 1, 2]);
  });

  it("uses 1/2/4 units for touch controls", () => {
    expect([oneButton, multipleButtons, fader].map((row) => playbackRowUnits(row, false))).toEqual([1, 2, 4]);
  });
});

const mocks = vi.hoisted(() => ({
  dispatch: vi.fn(), executeCommandLine: vi.fn(), refresh: vi.fn(), poolPlaybackAction: vi.fn(), resetCommandLine: vi.fn(), savePlaybackSlot: vi.fn(), clearPlaybackSlot: vi.fn(), storePlayback: vi.fn(),
  commandLine: "FIXTURE", error: null as string | null,
  hardwareConnected: false,
  state: { midiProfile: null, playbackColumns: 1, playbackRows: 1, playbackPage: 0, cueListSetTarget: 12 as number | null, cueListSetArmed: true, playbackSetArmed: false, shiftArmed: false, updateArmed: false, storeArmed: false, blackout: false },
  playbacks: {
    active_page: 1,
    pages: [{ number: 1, name: "Main", slots: {} as Record<string, number> }],
    pool: [] as Array<Record<string, any>>,
    active: [] as Array<Record<string, any>>,
    cue_lists: [{ id: "front", name: "Front sequence", cues: [] as Array<Record<string, any>>, mode: "sequence", priority: 0, looped: false }],
    desk: { buttons: 3 }, selected_playback: null as number | null,
  },
}));

vi.mock("../../api/ServerContext", () => ({ useServer: () => ({
  bootstrap: { hardware_connected: mocks.hardwareConnected }, playbacks: mocks.playbacks, groups: [], configuration: { speed_groups_bpm: [120, 90, 60, 30, 15], programmer_fade_millis: 3_000, sequence_master_fade_millis: 4_000 },
  commandLine: mocks.commandLine, error: mocks.error, resetCommandLine: mocks.resetCommandLine, executeCommandLine: mocks.executeCommandLine, refresh: mocks.refresh,
  poolPlaybackAction: mocks.poolPlaybackAction, savePlaybackSlot: mocks.savePlaybackSlot, clearPlaybackSlot: mocks.clearPlaybackSlot, storePlayback: mocks.storePlayback,
}) }));
vi.mock("../../state/AppContext", () => ({ useApp: () => ({ state: mocks.state, dispatch: mocks.dispatch }) }));

afterEach(cleanup);

describe("PlaybackFaderBank authoritative playback surfaces", () => {
  beforeEach(() => {
    mocks.dispatch.mockReset(); mocks.executeCommandLine.mockReset().mockResolvedValue(true); mocks.refresh.mockReset().mockResolvedValue(undefined); mocks.poolPlaybackAction.mockReset().mockResolvedValue(undefined); mocks.resetCommandLine.mockReset(); mocks.savePlaybackSlot.mockReset().mockResolvedValue(true); mocks.clearPlaybackSlot.mockReset().mockResolvedValue(true); mocks.storePlayback.mockReset().mockResolvedValue(undefined);
    mocks.commandLine = "FIXTURE"; mocks.error = null;
    mocks.hardwareConnected = false;
    Object.assign(mocks.state, { cueListSetTarget: 12, cueListSetArmed: true, playbackSetArmed: false, shiftArmed: false, updateArmed: false, storeArmed: false, blackout: false });
    mocks.playbacks.pages[0].slots = {}; mocks.playbacks.pool = []; mocks.playbacks.active = []; mocks.playbacks.selected_playback = null;
  });

  const assignPlayback = (overrides: Record<string, unknown> = {}) => {
    mocks.playbacks.pages[0].slots = { "1": 7 };
    mocks.playbacks.pool = [{ number: 7, name: "Front Wash", target: { type: "cue_list", cue_list_id: "front" }, buttons: ["go", "go_minus", "flash"], button_count: 3, fader: "master", has_fader: true, go_activates: true, auto_off: true, xfade_millis: 0, color: "#20c997", flash_release: "release_all", protect_from_swap: false, ...overrides }];
    Object.assign(mocks.state, { cueListSetTarget: null, cueListSetArmed: false });
  };

  it("projects arbitrary row starts and fills touch height with weighted tracks", () => {
    Object.assign(mocks.state, { cueListSetTarget: null, cueListSetArmed: false });
    const { container } = render(<PlaybackFaderBank playbackLayout={{
      playbacks_per_row: 1,
      rows: [
        { first_playback_slot: 1, has_fader: false, button_count: 1 },
        { first_playback_slot: 11, has_fader: false, button_count: 3 },
        { first_playback_slot: 21, has_fader: true, button_count: 1 },
      ],
    }} />);

    expect(container.querySelector(".playback-fader-bank")).toHaveStyle({
      gridTemplateRows: "minmax(0, 1fr) minmax(0, 2fr) minmax(0, 4fr)",
    });
    expect([...container.querySelectorAll("[data-playback-slot]")].map((element) => element.getAttribute("data-playback-slot"))).toEqual(["1", "11", "21"]);
  });

  it("fills hardware height with faderless and fader row weights", () => {
    mocks.hardwareConnected = true;
    Object.assign(mocks.state, { cueListSetTarget: null, cueListSetArmed: false });
    const { container } = render(<PlaybackFaderBank playbackLayout={{
      playbacks_per_row: 1,
      rows: [
        { first_playback_slot: 1, has_fader: false, button_count: 1 },
        { first_playback_slot: 11, has_fader: false, button_count: 3 },
        { first_playback_slot: 21, has_fader: true, button_count: 1 },
      ],
    }} />);
    expect(container.querySelector(".playback-fader-bank")).toHaveStyle({
      gridTemplateRows: "minmax(0, 1fr) minmax(0, 1fr) minmax(0, 2fr)",
    });
  });

  it("assigns the selected Cuelist source to the touched physical page slot", async () => {
    render(<PlaybackFaderBank count={1} />);
    fireEvent.click(screen.getByRole("button", { name: "Assign Cuelist 12 to page 1 playback 1" }));
    await waitFor(() => expect(mocks.executeCommandLine).toHaveBeenCalledWith("SET 12 AT 1.1"));
    expect(mocks.refresh).toHaveBeenCalledOnce();
    expect(mocks.dispatch).toHaveBeenCalledWith({ type: "SET_CUELIST_SET_ARMED", value: false });
  });

  it.each([
    ["software representation", () => screen.getByRole("button", { name: "Playback representation page 1 playback 1" })],
    ["top button", () => screen.getByRole("button", { name: "GO +" })],
    ["middle button", () => screen.getByRole("button", { name: "GO −" })],
    ["bottom button", () => screen.getByRole("button", { name: "FLASH" })],
    ["fader track and handle", () => screen.getByRole("slider", { name: "Master" })],
  ])("SET intercepts the %s without executing it and Cancel is inert", (_surface, target) => {
    assignPlayback(); mocks.state.playbackSetArmed = true;
    render(<PlaybackFaderBank count={1}/>);
    fireEvent.click(target());
    expect(screen.getByRole("dialog", { name: "Playback Configuration" })).toHaveAttribute("data-page", "1");
    expect(screen.getByRole("dialog", { name: "Playback Configuration" })).toHaveAttribute("data-slot", "1");
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(mocks.poolPlaybackAction).not.toHaveBeenCalled();
    expect(mocks.savePlaybackSlot).not.toHaveBeenCalled();
    expect(mocks.clearPlaybackSlot).not.toHaveBeenCalled();
  });

  it("opens an empty slot without fabricating a playback number and allocates only on Apply", async () => {
    Object.assign(mocks.state, { cueListSetTarget: null, cueListSetArmed: false, playbackSetArmed: true });
    render(<PlaybackFaderBank count={1}/>);
    fireEvent.click(screen.getByRole("button", { name: "Playback representation page 1 playback 1" }));
    expect(screen.getByText(/Empty slot/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Clear Playback" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Apply" }));
    await waitFor(() => expect(mocks.savePlaybackSlot).toHaveBeenCalledOnce());
    expect(mocks.savePlaybackSlot).toHaveBeenCalledWith(1, 1, expect.objectContaining({ number: 0, button_count: 3, has_fader: true }));
  });

  it("opens configuration when SHIFT is followed by the first playback button", () => {
    assignPlayback(); mocks.state.shiftArmed = true;
    render(<PlaybackFaderBank count={1}/>);
    fireEvent.click(screen.getByRole("button", { name: "GO +" }));
    expect(screen.getByRole("dialog", { name: "Playback Configuration" })).toBeInTheDocument();
    expect(mocks.poolPlaybackAction).not.toHaveBeenCalled();
  });

  it("routes Update before playback execution with concrete playback and current Cue context", () => {
    assignPlayback();
    mocks.state.updateArmed = true;
    mocks.playbacks.cue_lists[0].cues = [{ id: "cue-2", number: 2, name: "Look", fade_millis: 0, delay_millis: 0, trigger: { type: "manual" }, changes: [] }];
    mocks.playbacks.active = [{ playback_number: 7, cue_list_id: "front", cue_index: 0, paused: false, master: 1, flash: false }];
    const selected = vi.fn();
    window.addEventListener(UPDATE_TARGET_EVENT, selected);
    render(<PlaybackFaderBank count={1}/>);
    fireEvent.click(screen.getByRole("button", { name: "GO +" }));
    expect((selected.mock.calls[0][0] as CustomEvent).detail).toEqual({ family: { type: "cue" }, object_id: "front", playback_number: 7, cue_id: "cue-2", cue_number: 2, validate_active_context: true });
    expect(mocks.poolPlaybackAction).not.toHaveBeenCalled();
    window.removeEventListener(UPDATE_TARGET_EVENT, selected);
  });

  it("uses SELECT then any playback touch without firing the mapped button", async () => {
    assignPlayback(); mocks.commandLine = "SELECT";
    render(<PlaybackFaderBank count={1}/>);
    fireEvent.click(screen.getByRole("button", { name: "GO +" }));
    await waitFor(() => expect(mocks.poolPlaybackAction).toHaveBeenCalledWith(7, "select"));
    expect(mocks.poolPlaybackAction).toHaveBeenCalledTimes(1);
    expect(mocks.refresh).toHaveBeenCalledOnce(); expect(mocks.resetCommandLine).toHaveBeenCalledOnce();
  });

  it("makes the hardware card one display-only Cuelist selection surface", async () => {
    assignPlayback(); mocks.hardwareConnected = true;
    mocks.playbacks.cue_lists[0].cues = [{ id: "cue-1", number: 1, name: "Opening", fade_millis: 0, delay_millis: 0, trigger: { type: "manual" }, changes: [] }];
    const { container } = render(<PlaybackFaderBank count={1}/>);
    expect(screen.queryByRole("button", { name: /Playback representation/ })).not.toBeInTheDocument();
    const card = container.querySelector(".hardware-playback-card")!;
    fireEvent.click(card.querySelector("header b")!);
    await waitFor(() => expect(mocks.poolPlaybackAction).toHaveBeenCalledWith(7, "select"));
    expect(mocks.poolPlaybackAction).toHaveBeenCalledTimes(1);
    expect(mocks.dispatch).toHaveBeenCalledWith({ type: "OPEN_BUILTIN", kind: "cuelists" });
    expect(mocks.dispatch).toHaveBeenCalledWith({ type: "OPEN_BUILTIN_CUELIST", number: 7 });
  });

  it("does not select the hardware card when a real button or fader operates", () => {
    assignPlayback(); mocks.hardwareConnected = true;
    render(<PlaybackFaderBank count={1}/>);
    fireEvent.click(screen.getByRole("button", { name: "GO +" }));
    fireEvent.input(screen.getByRole("slider", { name: "Page 1 playback 1 fader" }), { target: { value: "42" } });
    expect(mocks.poolPlaybackAction).toHaveBeenCalledWith(7, "button", { button: 1, pressed: true, surface: "physical" });
    expect(mocks.poolPlaybackAction).toHaveBeenCalledWith(7, "master", { value: 0.42, surface: "physical" });
    expect(mocks.poolPlaybackAction).not.toHaveBeenCalledWith(7, "select");
  });

  it("records to the concrete hardware-card Cuelist and explicit page", async () => {
    assignPlayback(); mocks.hardwareConnected = true; mocks.state.storeArmed = true;
    mocks.playbacks.pages.push({ number: 3, name: "Page 3", slots: { "1": 7 } });
    const { container } = render(<PlaybackFaderBank pageNumber={3} count={1}/>);
    fireEvent.click(container.querySelector(".hardware-playback-card header")!);
    await waitFor(() => expect(mocks.storePlayback).toHaveBeenCalledWith(0, "front", 3));
    expect(mocks.poolPlaybackAction).toHaveBeenCalledWith(7, "select");
    expect(mocks.dispatch).toHaveBeenCalledWith({ type: "SET_STORE_ARMED", value: false });
    mocks.playbacks.pages.pop();
  });

  it("dispatches the authoritative button index, including held Flash lifetime", () => {
    assignPlayback(); const { container } = render(<PlaybackFaderBank count={1}/>);
    expect(container.querySelector(".vertical-touch-fader-stack")).toHaveClass("action-count-3");
    fireEvent.click(screen.getByRole("button", { name: "GO +" }));
    expect(mocks.poolPlaybackAction).toHaveBeenCalledWith(7, "button", { button: 1, pressed: true, surface: "physical" });
    const flash = screen.getByRole("button", { name: "FLASH" });
    fireEvent.pointerDown(flash, { pointerId: 4 }); fireEvent.pointerUp(flash, { pointerId: 4 });
    expect(mocks.poolPlaybackAction).toHaveBeenCalledWith(7, "button", { button: 3, pressed: true, surface: "physical" });
    expect(mocks.poolPlaybackAction).toHaveBeenCalledWith(7, "button", { button: 3, pressed: false, surface: "physical" });
  });

  it("omits disabled touch buttons while preserving configured button indices", () => {
    assignPlayback({ buttons: ["go", "none", "flash"], button_count: 3 });
    const { container } = render(<PlaybackFaderBank count={1}/>);
    expect(screen.queryByRole("button", { name: "DISABLED" })).not.toBeInTheDocument();
    expect(container.querySelector(".vertical-touch-fader-stack")).toHaveClass("action-count-2");
    expect(container.querySelectorAll(".vertical-touch-fader-actions .ui-button")).toHaveLength(2);
    fireEvent.pointerDown(screen.getByRole("button", { name: "FLASH" }), { pointerId: 4 });
    expect(mocks.poolPlaybackAction).toHaveBeenCalledWith(7, "button", { button: 3, pressed: true, surface: "physical" });
  });

  it("renders one configured faderless touch button as the only action", () => {
    assignPlayback({ buttons: ["flash", "none", "none"], button_count: 1, has_fader: false });
    const { container } = render(<PlaybackFaderBank count={1}/>);
    expect(container.querySelector(".faderless-playback-actions")).toHaveClass("action-count-1");
    expect(container.querySelectorAll(".faderless-playback-actions .ui-button")).toHaveLength(1);
    expect(screen.getByRole("button", { name: "FLASH" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "DISABLED" })).not.toBeInTheDocument();
  });

  it("dispatches TEMP as a press-to-toggle action on successive clicks", () => {
    assignPlayback({ buttons: ["temp", "none", "none"], button_count: 1, has_fader: false });
    render(<PlaybackFaderBank count={1}/>);
    const temp = screen.getByRole("button", { name: "TEMP" });
    fireEvent.click(temp);
    fireEvent.click(temp);
    expect(mocks.poolPlaybackAction).toHaveBeenNthCalledWith(1, 7, "button", { button: 1, pressed: true, surface: "physical" });
    expect(mocks.poolPlaybackAction).toHaveBeenNthCalledWith(2, 7, "button", { button: 1, pressed: true, surface: "physical" });
    expect(mocks.poolPlaybackAction).toHaveBeenCalledTimes(2);
  });

  it("renders only persisted controls for a one-button faderless playback", () => {
    assignPlayback({ buttons: ["toggle", "none", "none"], button_count: 1, has_fader: false });
    render(<PlaybackFaderBank count={1}/>);
    expect(screen.getByRole("button", { name: "TOGGLE" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "GO −" })).not.toBeInTheDocument();
    expect(screen.queryByRole("slider")).not.toBeInTheDocument();
  });

  it("shows X-fade direction/progress and safe-pickup feedback from runtime state", () => {
    assignPlayback({ fader: "x_fade" });
    mocks.playbacks.active = [{ playback_number: 7, cue_list_id: "front", cue_index: 0, current_cue_number: 1, effective_next_cue_number: 2, enabled: true, master: 1, flash: false, fader_position: .25, fader_pickup_required: true, manual_xfade_position: .25, manual_xfade_direction: "towards_high", manual_xfade_progress: .25 }];
    render(<PlaybackFaderBank count={1}/>);
    expect(screen.getByText("Pickup: lower to zero")).toBeInTheDocument();
    expect(screen.getByText("Cue 1 → 2 · 25%" )).toBeInTheDocument();
  });

  it("recognizes the marked click produced by a playback right-click", () => {
    assignPlayback(); const { container } = render(<PlaybackFaderBank count={1}/>);
    const click = new MouseEvent("click", { bubbles: true, cancelable: true }); Object.defineProperty(click, "lightSetShortcut", { value: true });
    fireEvent(container.querySelector("article")!, click);
    expect(screen.getByRole("dialog", { name: "Playback Configuration" })).toBeInTheDocument();
  });

  it("confirms atomic clear and retains the source object outside this UI operation", async () => {
    assignPlayback(); mocks.state.playbackSetArmed = true; render(<PlaybackFaderBank count={1}/>);
    fireEvent.click(screen.getByRole("button", { name: "Playback representation page 1 playback 1" }));
    fireEvent.click(screen.getByRole("button", { name: "Clear Playback" }));
    fireEvent.click(screen.getByRole("button", { name: "Keep Playback" }));
    expect(mocks.clearPlaybackSlot).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Clear Playback" }));
    fireEvent.click(screen.getByRole("button", { name: "Confirm Clear Playback" }));
    await waitFor(() => expect(mocks.clearPlaybackSlot).toHaveBeenCalledWith(1, 1));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Playback Configuration" })).not.toBeInTheDocument());
  });
});
