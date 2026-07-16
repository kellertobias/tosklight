import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { NumericPad, numericPadLayout } from "./NumericPad";

const dispatch = vi.fn((action: { type: string; value?: boolean }) => {
  if (action.type === "SET_SHIFT_ARMED") state.shiftArmed = Boolean(action.value);
});
const server = {
  bootstrap: { active_programmers: [] },
  session: { user: { id: "operator" } },
  selectedFixtures: [],
  configuration: { programmer_fade_millis: 3_000 },
  commandLine: "FIXTURE",
  commandTargetMode: "FIXTURE",
  commandLinePristine: true,
  resetCommandLine: vi.fn(),
  setSelection: vi.fn(),
  clearProgrammerValues: vi.fn(),
  clearProgrammer: vi.fn(),
  undoProgrammer: vi.fn(),
  executeCommandLine: vi.fn().mockResolvedValue(true),
  setCommandLine: vi.fn(),
  setControlTiming: vi.fn(),
  playbacks: { selected_playback: 42, active: [{ playback_number: 7, cue_list_id: "running", cue_index: 0, paused: false, master: 1, flash: false }] },
};
const state = {
  storeArmed: false,
  cueListSetArmed: false,
  preload: "idle",
  builtIn: null,
  activeDeskId: "programming",
  desks: [
    { id: "desk-one", name: "Desk One", panes: [] },
    { id: "desk-two", name: "Desk Two", panes: [] },
    { id: "desk-three", name: "Desk Three", panes: [] },
  ],
  patchSetArmed: false,
  presetSetArmed: false,
  shiftArmed: false,
};

vi.mock("../../api/ServerContext", () => ({ useServer: () => server }));
vi.mock("../../state/AppContext", () => ({ useApp: () => ({ state, dispatch }) }));

afterEach(() => { cleanup(); state.shiftArmed = false; vi.clearAllMocks(); });

describe("NumericPad layout", () => {
  it("uses separate 5-by-2 and 5-by-4 grids with a spanning fade and single-row Through and Enter keys", () => {
    const { container } = render(<NumericPad/>);
    expect(container.querySelector(".numeric-pad-command-section")).toBeInTheDocument();
    expect(container.querySelector(".numeric-pad-number-section")).toBeInTheDocument();
    expect(container.querySelector(".numeric-pad-fade")).toHaveStyle({ gridColumn: "1 / span 2", gridRow: "1" });
    for (const { key, section, column, row, rowSpan = 1 } of numericPadLayout) {
      const expectedColumn = section === "commands" ? column : column - 3;
      expect(container.querySelector(`[data-keypad-key="${key}"]`)).toHaveStyle({
        gridColumn: `${expectedColumn}`,
        gridRow: `${row} / span ${rowSpan}`,
      });
    }
    expect(screen.getByRole("button", { name: "SET" })).toHaveAttribute("data-keypad-key", "SET");
    expect(screen.getByRole("button", { name: "CUE" })).toHaveAttribute("data-keypad-key", "CUE");
    expect(screen.getByRole("button", { name: "UND" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "TRU" })).toHaveStyle({ gridColumn: "4", gridRow: "4 / span 1" });
    expect(screen.getByRole("button", { name: "ENT" })).toHaveStyle({ gridColumn: "4", gridRow: "5 / span 1" });
  });

  it("routes Shift shortcuts to built-ins, the explicitly selected playback, and stored desks", () => {
    render(<NumericPad/>);
    const shifted = (key: string) => {
      fireEvent.click(screen.getByRole("button", { name: "SHIFT" }));
      fireEvent.click(screen.getByRole("button", { name: key }));
    };

    shifted(".");
    expect(dispatch).toHaveBeenCalledWith({ type: "OPEN_BUILTIN", kind: "help" });
    shifted("0");
    expect(dispatch).toHaveBeenCalledWith({ type: "OPEN_BUILTIN", kind: "fixtures" });
    shifted("1");
    expect(dispatch).toHaveBeenCalledWith({ type: "OPEN_BUILTIN", kind: "groups" });
    shifted("2");
    expect(dispatch).toHaveBeenCalledWith({ type: "SET_PRESET_FAMILY", family: "All" });
    expect(dispatch).toHaveBeenCalledWith({ type: "OPEN_BUILTIN", kind: "presets" });
    shifted("3");
    expect(dispatch).toHaveBeenCalledWith({ type: "OPEN_BUILTIN", kind: "cuelists" });
    shifted("4");
    expect(dispatch).toHaveBeenCalledWith({ type: "OPEN_BUILTIN_CUELIST", number: 42 });
    shifted("5");
    expect(dispatch).toHaveBeenCalledWith({ type: "OPEN_BUILTIN", kind: "dynamics" });
    shifted("6");
    expect(dispatch).toHaveBeenCalledWith({ type: "OPEN_BUILTIN", kind: "channels" });
    shifted("7");
    shifted("8");
    shifted("9");
    expect(dispatch).toHaveBeenCalledWith({ type: "OPEN_DESK", id: "desk-one" });
    expect(dispatch).toHaveBeenCalledWith({ type: "OPEN_DESK", id: "desk-two" });
    expect(dispatch).toHaveBeenCalledWith({ type: "OPEN_DESK", id: "desk-three" });
    shifted("CLR");
    expect(dispatch).toHaveBeenCalledWith({ type: "SET_MODAL", modal: "systemControlsOpen", value: true });
    shifted("DEL");
    expect(dispatch).toHaveBeenCalledWith({ type: "SET_MODAL", modal: "systemControlsOpen", value: true });
  });
});
