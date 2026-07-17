import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Button } from "../common";
import { CommandLineBar } from "./CommandLineBar";
import { UPDATE_SETTINGS_EVENT, UPDATE_TARGET_MENU_EVENT } from "./updateWorkflow";

const state = {
  midiProfile: false,
  controlMode: "programmer",
  preload: "idle",
  preloadActive: false,
  updateArmed: false,
  storeArmed: false,
  shiftArmed: false,
  cueListSetArmed: false,
  playbackSetArmed: false,
  presetSetArmed: false,
  regularNumberShortcuts: true,
  playbackPage: 0,
  playbackPageNames: ["Main"],
  blackout: false,
  builtIn: null as string | null,
  patchSetArmed: false,
};
const dispatch = vi.fn((action: { type: string; value?: boolean }) => {
  if (action.type === "SET_UPDATE_ARMED") state.updateArmed = Boolean(action.value);
  if (action.type === "SET_STORE_ARMED") state.storeArmed = Boolean(action.value);
  if (action.type === "SET_SHIFT_ARMED") state.shiftArmed = Boolean(action.value);
});
const server = {
  bootstrap: { hardware_connected: false, active_programmers: [], frame_rate_hz: 60, active_timecode: null as string | null },
  session: { session_id: "session-a" },
  selectedFixtures: [],
  playbacks: null,
  commandLine: "FIXTURE",
  commandTargetMode: "FIXTURE",
  commandLinePristine: true,
  error: null,
  status: "connected",
  poolPlaybackAction: vi.fn(),
  setPlaybackPage: vi.fn(),
  preloadAction: vi.fn(),
  executeCommandLine: vi.fn().mockResolvedValue(true),
  setCommandLine: vi.fn((value: string) => { server.commandLine = value; }),
  resetCommandLine: vi.fn(),
  dismissError: vi.fn(),
};

vi.mock("../../api/ServerContext", () => ({ useServer: () => server }));
vi.mock("../../state/AppContext", () => ({ useApp: () => ({ state, dispatch }) }));

beforeEach(() => {
  vi.useFakeTimers();
  state.updateArmed = false;
  state.storeArmed = false;
  state.shiftArmed = false;
  state.builtIn = null;
  state.patchSetArmed = false;
  state.midiProfile = false;
  state.regularNumberShortcuts = true;
  server.commandLine = "FIXTURE";
  server.bootstrap.active_timecode = null;
  vi.clearAllMocks();
});

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

describe("Shift+Record Update gestures", () => {
  it("uses gray for no timecode and blue only for a present timecode", () => {
    const { rerender } = render(<CommandLineBar/>);
    expect(screen.getByText("No Timecode").closest(".timecode-status")).toHaveClass("timecode-idle");

    server.bootstrap.active_timecode = "01:02:03:04";
    rerender(<CommandLineBar/>);
    expect(screen.getByText("01:02:03:04")).toHaveClass("timecode-active");
  });

  it("keeps the command-to-REC/Preload space free of Highlight status panels in both layouts", () => {
    const { container, rerender } = render(<CommandLineBar/>);
    const assertNoHighlightPanel = () => {
      const commandField = container.querySelector(".command-field");
      const recordPreload = container.querySelector(".command-record-preload");
      expect(commandField?.nextElementSibling).toBe(recordPreload);
      expect(container.querySelector('[aria-label="Highlight status"]')).not.toBeInTheDocument();
      expect(container.querySelector(".highlight-feedback,.command-highlight-feedback")).not.toBeInTheDocument();
    };
    assertNoHighlightPanel();

    state.midiProfile = true;
    rerender(<CommandLineBar/>);
    assertNoHighlightPanel();
  });

  it("routes the physical Home-key SET shortcut through the selected Patch control surface", () => {
    state.builtIn = "patch";
    const set = vi.fn();
    render(<><Button data-keypad-key="SET" onClick={set}>SET target</Button><CommandLineBar/></>);

    fireEvent.keyDown(window, { code: "Home", key: "Home" });

    expect(set).toHaveBeenCalledOnce();
    expect(server.setCommandLine).not.toHaveBeenCalled();
  });

  it("disables the complete software keyboard shortcut layer", () => {
    state.builtIn = "patch";
    state.regularNumberShortcuts = false;
    const set = vi.fn();
    render(<><Button data-keypad-key="SET" onClick={set}>SET target</Button><CommandLineBar/></>);

    fireEvent.keyDown(window, { code: "Home", key: "Home" });
    fireEvent.keyDown(window, { code: "Delete", key: "Delete", shiftKey: true });

    expect(set).not.toHaveBeenCalled();
    expect(dispatch).not.toHaveBeenCalledWith({ type: "SET_MODAL", modal: "systemControlsOpen", value: true });
    expect(server.setCommandLine).not.toHaveBeenCalled();
  });

  it("keeps single, second-press, and long-press software gestures mutually exclusive", () => {
    const menu = vi.fn();
    const settings = vi.fn();
    window.addEventListener(UPDATE_TARGET_MENU_EVENT, menu);
    window.addEventListener(UPDATE_SETTINGS_EVENT, settings);
    state.shiftArmed = true;
    render(<CommandLineBar/>);
    const record = screen.getByRole("button", { name: "REC" });

    fireEvent.pointerDown(record);
    fireEvent.pointerUp(record);
    fireEvent.click(record);
    expect(dispatch).toHaveBeenCalledWith({ type: "SET_UPDATE_ARMED", value: true });
    expect(server.setCommandLine).toHaveBeenCalledWith("UPDATE ", false);
    expect(menu).not.toHaveBeenCalled();

    fireEvent.pointerDown(record);
    fireEvent.pointerUp(record);
    fireEvent.click(record);
    expect(menu).toHaveBeenCalledTimes(1);

    state.updateArmed = false;
    fireEvent.pointerDown(record);
    vi.advanceTimersByTime(650);
    fireEvent.pointerUp(record);
    fireEvent.click(record);
    expect(settings).toHaveBeenCalledTimes(1);
    expect(dispatch.mock.calls.filter(([action]) => action.type === "SET_UPDATE_ARMED" && action.value === true)).toHaveLength(1);

    window.removeEventListener(UPDATE_TARGET_MENU_EVENT, menu);
    window.removeEventListener(UPDATE_SETTINGS_EVENT, settings);
  });

  it("uses the same exclusive gestures for Shift+End on a software-only desk", () => {
    const menu = vi.fn();
    const settings = vi.fn();
    window.addEventListener(UPDATE_TARGET_MENU_EVENT, menu);
    window.addEventListener(UPDATE_SETTINGS_EVENT, settings);
    render(<CommandLineBar/>);

    fireEvent.keyDown(window, { code: "End", key: "End", shiftKey: true });
    vi.advanceTimersByTime(100);
    fireEvent.keyUp(window, { code: "End", key: "End", shiftKey: true });
    expect(state.updateArmed).toBe(true);

    fireEvent.keyDown(window, { code: "End", key: "End", shiftKey: true });
    fireEvent.keyUp(window, { code: "End", key: "End", shiftKey: true });
    expect(menu).toHaveBeenCalledTimes(1);

    state.updateArmed = false;
    fireEvent.keyDown(window, { code: "End", key: "End", shiftKey: true });
    vi.advanceTimersByTime(650);
    fireEvent.keyUp(window, { code: "End", key: "End", shiftKey: true });
    expect(settings).toHaveBeenCalledTimes(1);
    expect(state.updateArmed).toBe(false);

    window.removeEventListener(UPDATE_TARGET_MENU_EVENT, menu);
    window.removeEventListener(UPDATE_SETTINGS_EVENT, settings);
  });
});
