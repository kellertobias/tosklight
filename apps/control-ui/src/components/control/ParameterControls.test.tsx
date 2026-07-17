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
  selectedFixtures: [] as string[],
  selectedGroupId: null as string | null,
  groups: [] as any[],
  patch: { fixtures: [] as any[] },
  bootstrap: { active_programmers: [] as any[] },
  session: { session_id: "session-1", user: { id: "operator" } },
  readVisualization: vi.fn().mockResolvedValue({ values: [] }),
  alignSelection: vi.fn(),
  setProgrammer: vi.fn(),
  setGroupValue: vi.fn(),
  releaseProgrammer: vi.fn(),
  releaseGroupValue: vi.fn(),
};

vi.mock("../../state/AppContext", () => ({ useApp: () => ({ state, dispatch }) }));
vi.mock("../../api/ServerContext", () => ({ useServer: () => server }));

afterEach(() => {
  cleanup();
  state.shiftArmed = false;
  server.selectedFixtures = [];
  server.selectedGroupId = null;
  server.groups = [];
  server.patch.fixtures = [];
  server.bootstrap.active_programmers = [];
  vi.clearAllMocks();
});

describe("ParameterControls alignment", () => {
  it("releases only the visible fixture-scoped attribute", () => {
    server.selectedFixtures = ["fixture-1"];
    server.patch.fixtures = [{
      fixture_id: "fixture-1",
      logical_heads: [],
      definition: { heads: [{ shared: true, parameters: [{ attribute: "intensity", capabilities: [] }] }] },
    }];
    server.bootstrap.active_programmers = [{
      session_id: "session-1",
      user_id: "operator",
      values: [{ fixture_id: "fixture-1", attribute: "intensity" }],
      group_values: {},
    }];
    render(<ParameterControls />);
    fireEvent.click(screen.getByRole("button", { name: "Release Dimmer" }));
    expect(server.releaseProgrammer).toHaveBeenCalledWith("fixture-1", "intensity");
    expect(server.releaseGroupValue).not.toHaveBeenCalled();
  });

  it("shows the fixture programmer target while visualization is still fading", async () => {
    server.selectedFixtures = ["fixture-1"];
    server.patch.fixtures = [{
      fixture_id: "fixture-1",
      logical_heads: [],
      definition: { heads: [{ shared: true, parameters: [{ attribute: "intensity", capabilities: [] }] }] },
    }];
    server.bootstrap.active_programmers = [{
      session_id: "session-1",
      user_id: "operator",
      values: [{
        fixture_id: "fixture-1",
        attribute: "intensity",
        value: { kind: "normalized", value: 1 },
      }],
      group_values: {},
    }];
    server.readVisualization.mockResolvedValue({
      values: [{ fixture_id: "fixture-1", attribute: "intensity", value: { kind: "normalized", value: 0 } }],
    });

    render(<ParameterControls />);

    expect(await screen.findByText("100%")).toBeInTheDocument();
  });

  it("shows the Group programmer target while its members are still fading", async () => {
    server.selectedFixtures = ["fixture-1"];
    server.selectedGroupId = "3";
    server.groups = [{ id: "3", body: { programming: {}, fixtures: ["fixture-1"] } }];
    server.patch.fixtures = [{
      fixture_id: "fixture-1",
      logical_heads: [],
      definition: { heads: [{ shared: true, parameters: [{ attribute: "intensity", capabilities: [] }] }] },
    }];
    server.bootstrap.active_programmers = [{
      session_id: "session-1",
      user_id: "operator",
      values: [],
      group_values: {
        "3": {
          intensity: {
            value: { kind: "normalized", value: 0.75 },
          },
        },
      },
    }];
    server.readVisualization.mockResolvedValue({
      values: [{ fixture_id: "fixture-1", attribute: "intensity", value: { kind: "normalized", value: 0 } }],
    });

    render(<ParameterControls />);

    expect(await screen.findByText("75%")).toBeInTheDocument();
  });

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
