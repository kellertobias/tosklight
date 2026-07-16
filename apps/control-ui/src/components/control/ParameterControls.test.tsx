import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { ParameterControls } from "./ParameterControls";

const state = {
  stageMode: "select",
  builtIn: null,
  desks: [],
  activeDeskId: "programming",
  preload: "idle",
  shiftArmed: false,
};
const dispatch = vi.fn((action: { type: string; value?: boolean }) => {
  if (action.type === "SET_SHIFT_ARMED") state.shiftArmed = Boolean(action.value);
});
const server = {
  selectedFixtures: [],
  selectedGroupId: null,
  patch: { fixtures: [] },
  readVisualization: vi.fn(),
  alignSelection: vi.fn(),
};

vi.mock("../../state/AppContext", () => ({ useApp: () => ({ state, dispatch }) }));
vi.mock("../../api/ServerContext", () => ({ useServer: () => server }));

afterEach(() => {
  cleanup();
  state.shiftArmed = false;
  vi.clearAllMocks();
});

describe("ParameterControls alignment", () => {
  it("starts off, cycles Out, Center, Left, Right, and Shift+Align turns it off", () => {
    render(<ParameterControls />);
    fireEvent.click(screen.getByRole("button", { name: "Position" }));
    const align = screen.getByRole("button", { name: "Align Off" });
    expect(align).toHaveClass("align-off");

    for (const mode of ["out", "center", "left", "right"] as const) {
      fireEvent.click(align);
      expect(server.alignSelection).toHaveBeenLastCalledWith("pan", mode);
      expect(align).toHaveAccessibleName(`Align ${mode[0].toUpperCase()}${mode.slice(1)}`);
      expect(align).toHaveClass("align-active");
    }

    state.shiftArmed = true;
    fireEvent.click(align);
    expect(align).toHaveAccessibleName("Align Off");
    expect(align).toHaveClass("align-off");
    expect(dispatch).toHaveBeenCalledWith({ type: "SET_SHIFT_ARMED", value: false });
    expect(server.alignSelection).toHaveBeenCalledTimes(4);
  });
});
