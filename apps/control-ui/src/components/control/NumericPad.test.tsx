import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { NumericPad, numericPadLayout } from "./NumericPad";

const dispatch = vi.fn((action: { type: string; value?: boolean }) => {
  if (action.type === "SET_SHIFT_ARMED") state.shiftArmed = Boolean(action.value);
});
const server = {
  bootstrap: { active_programmers: [] as Array<Record<string, unknown>> },
  session: { user: { id: "operator" } },
  selectedFixtures: [] as string[],
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
  builtIn: null as string | null,
  activeDeskId: "programming",
  desks: [
    { id: "desk-one", name: "Desk One", panes: [] },
    { id: "desk-two", name: "Desk Two", panes: [] },
    { id: "desk-three", name: "Desk Three", panes: [] },
  ],
  patchSetArmed: false,
  presetSetArmed: false,
  playbackSetArmed: false,
  shiftArmed: false,
};

vi.mock("../../api/ServerContext", () => ({ useServer: () => server }));
vi.mock("../../state/AppContext", () => ({ useApp: () => ({ state, dispatch }) }));

afterEach(() => {
  cleanup();
  server.bootstrap.active_programmers = [];
  server.selectedFixtures = [];
  server.commandLine = "FIXTURE";
  server.commandLinePristine = true;
  state.shiftArmed = false;
  state.activeDeskId = "programming";
  (state as { builtIn: string | null }).builtIn = null;
  (state.desks[0] as { panes: Array<{ kind: string }> }).panes = [];
  vi.clearAllMocks();
});

describe("NumericPad layout", () => {
  it("shows dark, lit, and blinking Clear states and clears selection before values", () => {
    const { rerender } = render(<NumericPad/>);
    const clear = () => screen.getByRole("button", { name: "CLR" });
    expect(clear()).toHaveClass("clear-idle");

    server.selectedFixtures = ["fixture-1"];
    rerender(<NumericPad/>);
    expect(clear()).toHaveClass("clear-active");
    fireEvent.click(clear());
    expect(server.setSelection).toHaveBeenCalledWith([]);
    expect(server.clearProgrammerValues).not.toHaveBeenCalled();

    server.selectedFixtures = [];
    server.bootstrap.active_programmers = [{
      user_id: "operator",
      values: [{ fixture_id: "fixture-1", attribute: "intensity", value: { kind: "normalized", value: 0.5 } }],
      group_values: {},
    }];
    rerender(<NumericPad/>);
    expect(clear()).toHaveClass("clear-warning");
    fireEvent.click(clear());
    expect(server.clearProgrammerValues).toHaveBeenCalledTimes(1);

    server.bootstrap.active_programmers = [];
    rerender(<NumericPad/>);
    expect(clear()).toHaveClass("clear-idle");
  });

  it("arms playback configuration when a Virtual Playback grid is the available target surface", () => {
    const grid = document.createElement("div");
    grid.className = "virtual-playback-grid";
    document.body.append(grid);
    render(<NumericPad/>);
    fireEvent.click(screen.getByRole("button", { name: "SET" }));
    expect(dispatch).toHaveBeenCalledWith({ type: "SET_PLAYBACK_SET_ARMED", value: true });
    grid.remove();
  });

  it("arms the same selected Patch target from the software SET key", () => {
    state.builtIn = "patch";
    render(<NumericPad/>);

    fireEvent.click(screen.getByRole("button", { name: "SET" }));

    expect(dispatch).toHaveBeenCalledWith({ type: "SET_PATCH_ARMED", value: true });
    expect(server.setCommandLine).not.toHaveBeenCalled();
  });

  it("does not let a hidden Presets pane steal SET from another visible built-in", () => {
    state.activeDeskId = "desk-one";
    (state as { builtIn: string | null }).builtIn = "groups";
    (state.desks[0] as { panes: Array<{ kind: string }> }).panes = [{ kind: "presets" }];
    render(<NumericPad/>);
    fireEvent.click(screen.getByRole("button", { name: "SET" }));
    expect(dispatch).not.toHaveBeenCalledWith({ type: "SET_PRESET_SET_ARMED", value: true });
    expect(server.setCommandLine).toHaveBeenCalledWith("SET", false);
  });

  it("keeps SET as a command token once Copy or Move entry has started", () => {
    state.activeDeskId = "desk-one";
    (state.desks[0] as { panes: Array<{ kind: string }> }).panes = [{ kind: "presets" }];
    server.commandLine = "COPY";
    server.commandLinePristine = false;
    render(<NumericPad/>);

    fireEvent.click(screen.getByRole("button", { name: "SET" }));
    expect(dispatch).not.toHaveBeenCalledWith({ type: "SET_PRESET_SET_ARMED", value: true });
    expect(server.setCommandLine).toHaveBeenCalledWith("COPY SET ", false);
  });

  it("uses separate six-row command and number grids with Highlight above Group and a single-row Enter key", () => {
    const { container } = render(<NumericPad/>);
    expect(container.querySelector(".numeric-pad-command-section")).toBeInTheDocument();
    expect(container.querySelector(".numeric-pad-number-section")).toBeInTheDocument();
    expect(container.querySelector(".numeric-pad-fade")).toHaveStyle({ gridColumn: "1 / span 2", gridRow: "1" });
    for (const { key, section, column, row, rowSpan = 1 } of numericPadLayout) {
      const expectedColumn = section === "commands" ? column : column - 3;
      const expectedRow = row + (section === "numbers" ? 1 : 0);
      expect(container.querySelector(`[data-keypad-key="${key}"]`)).toHaveStyle({
        gridColumn: `${expectedColumn}`,
        gridRow: `${expectedRow} / span ${rowSpan}`,
      });
    }
    const highlight = screen.getByRole("region", { name: "Highlight and step through" });
    expect(highlight.parentElement).toHaveClass("numeric-pad-number-section");
    expect(highlight.querySelector(".highlight-toggle")).toHaveTextContent("HIGH");
    expect(highlight.querySelector(".highlight-previous")).toHaveTextContent("PREV");
    expect(highlight.querySelector(".highlight-next")).toHaveTextContent("NEXT");
    expect(highlight.querySelector(".highlight-capture")).toHaveTextContent("ALL");
    expect(screen.getByRole("button", { name: "SET" })).toHaveAttribute("data-keypad-key", "SET");
    expect(screen.getByRole("button", { name: "CUE" })).toHaveAttribute("data-keypad-key", "CUE");
    expect(screen.getByRole("button", { name: "UND" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "TRU" })).toHaveStyle({ gridColumn: "4", gridRow: "5 / span 1" });
    expect(screen.getByRole("button", { name: "ENT" })).toHaveStyle({ gridColumn: "4", gridRow: "6 / span 1" });
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
    shifted("TIME");
    expect(server.setCommandLine).toHaveBeenCalledWith("SPD GRP", false);
    server.commandLine = "SPD GRP 1 AT";
    server.commandLinePristine = false;
    shifted("TIME");
    expect(server.setCommandLine).toHaveBeenCalledWith("SPD GRP 1 AT SPD GRP", false);
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
