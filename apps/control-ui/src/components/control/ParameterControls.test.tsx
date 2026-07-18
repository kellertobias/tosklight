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
  bootstrap: { active_programmers: [] as any[], hardware_connected: false },
  session: { session_id: "session-1", user: { id: "operator" } },
  readVisualization: vi.fn().mockResolvedValue({ values: [] }),
  alignSelection: vi.fn(),
  setProgrammer: vi.fn(),
  setProgrammerMany: vi.fn(),
  setProgrammerValue: vi.fn(),
  controlFixtureAction: vi.fn(),
  generateFixturePresets: vi.fn().mockResolvedValue({ created: [] }),
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
  server.bootstrap.hardware_connected = false;
  vi.clearAllMocks();
});

function schemaV2Fixture(): any {
  return {
    fixture_id: "fixture-1",
    logical_heads: [],
    definition: {
      mode_id: "mode-1",
      heads: [{ shared: true, parameters: [{ attribute: "gobo.1", capabilities: [] }] }],
      profile_snapshot: {
        id: "profile-1",
        modes: [{
          id: "mode-1",
          heads: [{ id: "head-1", master_shared: true }],
          channels: [{
            id: "channel-1",
            head_id: "head-1",
            attribute: "gobo.1",
            functions: [{
              id: "function-1",
              attribute: "gobo.1",
              behavior: {
                type: "indexed",
                semantic_id: "gobo.dots",
                label: "Dots",
                raw_value: 93,
              },
            }],
          }],
          control_actions: [{
            id: "action-1",
            name: "Lamp reset",
            kind: "momentary",
            duration_millis: null,
            assignments: [{ channel_id: "channel-1", active_raw: 255, inactive_raw: 0 }],
          }],
        }],
      },
    },
  };
}

describe("ParameterControls alignment", () => {
  it("keeps six numbered hardware feedback slots and routes fine and press-turn changes", () => {
    server.bootstrap.hardware_connected = true;
    server.selectedFixtures = ["fixture-1"];
    server.patch.fixtures = [{
      fixture_id: "fixture-1",
      logical_heads: [],
      definition: { heads: [{ shared: true, parameters: [{ attribute: "intensity", capabilities: [] }] }] },
    }];
    render(<ParameterControls />);

    expect(screen.getByLabelText("Encoder 1: Dimmer, 0%")).toBeInTheDocument();
    for (let slot = 2; slot <= 6; slot += 1) expect(screen.getByLabelText(`Encoder ${slot} unassigned`)).toBeInTheDocument();
    expect(screen.queryByRole("slider")).not.toBeInTheDocument();

    window.dispatchEvent(new CustomEvent("light:encoder-action", { detail: { control: "encode/1", value: "up" } }));
    expect(server.setProgrammer).toHaveBeenLastCalledWith("fixture-1", "intensity", .01);
    window.dispatchEvent(new CustomEvent("light:encoder-action", { detail: { control: "encode/1", value: "right" } }));
    expect(server.setProgrammer).toHaveBeenLastCalledWith("fixture-1", "intensity", .1);
  });

  it("uses the hardware encoder card itself as the set-value target", () => {
    server.bootstrap.hardware_connected = true;
    server.selectedFixtures = ["fixture-1"];
    server.patch.fixtures = [{
      fixture_id: "fixture-1",
      logical_heads: [],
      definition: { heads: [{ shared: true, parameters: [{ attribute: "intensity", capabilities: [] }] }] },
    }];
    render(<ParameterControls />);

    expect(screen.queryByRole("button", { name: "Set value for Dimmer" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Encoder 1: Dimmer, 0%" }));

    expect(screen.getByRole("dialog", { name: "Encoder 1 value" })).toBeInTheDocument();
  });

  it("spreads a typed hardware encoder range over the ordered fixture selection", () => {
    server.bootstrap.hardware_connected = true;
    server.selectedFixtures = ["fixture-3", "fixture-1", "fixture-2"];
    server.patch.fixtures = server.selectedFixtures.map((fixture_id) => ({
      fixture_id,
      logical_heads: [],
      definition: { heads: [{ shared: true, parameters: [{ attribute: "intensity", capabilities: [] }] }] },
    }));
    render(<ParameterControls />);

    fireEvent.click(screen.getByRole("button", { name: "Encoder 1: Dimmer, 0%" }));
    for (const key of ["0", "THRU", "5", "0", "ENTER"]) {
      fireEvent.click(screen.getByRole("button", { name: key }));
    }

    expect(server.setProgrammerMany).toHaveBeenCalledWith([
      { fixtureId: "fixture-3", attribute: "intensity", value: 0 },
      { fixtureId: "fixture-1", attribute: "intensity", value: 0.25 },
      { fixtureId: "fixture-2", attribute: "intensity", value: 0.5 },
    ]);
  });

  it("clears all physical encoder mappings in Direct mode", () => {
    server.bootstrap.hardware_connected = true;
    server.selectedFixtures = ["fixture-1"];
    server.patch.fixtures = [schemaV2Fixture()];
    render(<ParameterControls />);
    fireEvent.click(screen.getByRole("button", { name: "Direct values and actions" }));
    for (let slot = 1; slot <= 6; slot += 1) expect(screen.getByLabelText(`Encoder ${slot} unassigned`)).toBeInTheDocument();
    expect(screen.queryByLabelText(/Encoder 1:/)).not.toBeInTheDocument();
  });

  it("formats a discrete hardware target as a semantic value instead of a percentage", () => {
    server.bootstrap.hardware_connected = true;
    server.selectedFixtures = ["fixture-1"];
    server.patch.fixtures = [{
      fixture_id: "fixture-1",
      logical_heads: [],
      definition: { heads: [{ shared: true, parameters: [{ attribute: "control.reset", capabilities: [] }] }] },
    }];
    server.bootstrap.active_programmers = [{
      session_id: "session-1",
      values: [{ fixture_id: "fixture-1", attribute: "control.reset", value: { kind: "discrete", value: "fixture.reset.safe" } }],
      group_values: {},
    }];
    render(<ParameterControls />);
    fireEvent.click(screen.getByRole("button", { name: "Control" }));
    expect(screen.getByLabelText("Encoder 1: control reset, fixture.reset.safe")).not.toHaveTextContent("Built-in");
    expect(screen.queryByRole("button", { name: "Set value for control reset" })).not.toBeInTheDocument();
  });

  it("shows a hardware encoder percentage range for mixed selected fixture values", async () => {
    server.bootstrap.hardware_connected = true;
    server.selectedFixtures = ["fixture-1", "fixture-2"];
    server.patch.fixtures = [
      {
        fixture_id: "fixture-1",
        logical_heads: [],
        definition: { heads: [{ shared: true, parameters: [{ attribute: "intensity", capabilities: [] }] }] },
      },
      {
        fixture_id: "fixture-2",
        logical_heads: [],
        definition: { heads: [{ shared: true, parameters: [{ attribute: "intensity", capabilities: [] }] }] },
      },
    ];
    server.readVisualization.mockResolvedValue({
      values: [
        { fixture_id: "fixture-1", attribute: "intensity", value: { kind: "normalized", value: 0.25 } },
        { fixture_id: "fixture-2", attribute: "intensity", value: { kind: "normalized", value: 0.75 } },
      ],
    });

    render(<ParameterControls />);

    const encoder = await screen.findByLabelText("Encoder 1: Dimmer, 25%...75%");
    expect(encoder).toHaveTextContent("Dimmer");
    expect(encoder).toHaveTextContent("Enc 1");
    expect(encoder).not.toHaveTextContent("Turn");
    expect(encoder).not.toHaveTextContent("Intensity");
  });

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

describe("ParameterControls schema-v2 direct picker", () => {
  it("programs indexed values by stable semantic ID", () => {
    server.selectedFixtures = ["fixture-1"];
    server.patch.fixtures = [schemaV2Fixture()];

    render(<ParameterControls />);
    fireEvent.click(screen.getByRole("button", { name: "Direct values and actions" }));
    fireEvent.click(screen.getByRole("button", { name: "Dots indexed value" }));

    expect(server.setProgrammerValue).toHaveBeenCalledWith(
      "fixture-1",
      "gobo.1",
      { kind: "discrete", value: "gobo.dots" },
    );
  });

  it("holds and releases every assignment through one typed momentary action", () => {
    server.selectedFixtures = ["fixture-1"];
    server.patch.fixtures = [schemaV2Fixture()];

    render(<ParameterControls />);
    fireEvent.click(screen.getByRole("button", { name: "Direct values and actions" }));
    const action = screen.getByRole("button", { name: "Lamp reset momentary control action" });
    fireEvent.pointerDown(action, { pointerId: 7 });
    fireEvent.pointerUp(action, { pointerId: 7 });

    expect(server.controlFixtureAction.mock.calls).toEqual([
      ["fixture-1", "action-1", true],
      ["fixture-1", "action-1", false],
    ]);
  });

  it("toggles latched actions and lets the server own timed-pulse release", () => {
    const fixture = schemaV2Fixture();
    fixture.definition.profile_snapshot.modes[0].control_actions = [
      {
        id: "action-latched",
        name: "Lamp power",
        kind: "latched",
        duration_millis: null,
        assignments: [{ channel_id: "channel-1", active_raw: 255, inactive_raw: 0 }],
      },
      {
        id: "action-pulse",
        name: "Fixture reset",
        kind: "timed_pulse",
        duration_millis: 750,
        assignments: [{ channel_id: "channel-1", active_raw: 255, inactive_raw: 0 }],
      },
    ];
    server.selectedFixtures = ["fixture-1"];
    server.patch.fixtures = [fixture];

    render(<ParameterControls />);
    fireEvent.click(screen.getByRole("button", { name: "Direct values and actions" }));
    const latched = screen.getByRole("button", { name: "Lamp power latched control action" });
    fireEvent.click(latched);
    fireEvent.click(latched);
    fireEvent.click(screen.getByRole("button", { name: "Fixture reset timed_pulse control action" }));

    expect(server.controlFixtureAction.mock.calls).toEqual([
      ["fixture-1", "action-latched", true],
      ["fixture-1", "action-latched", false],
      ["fixture-1", "action-pulse", true],
    ]);
  });

  it("creates portable presets only after the explicit operator action", async () => {
    server.selectedFixtures = ["fixture-1"];
    server.patch.fixtures = [schemaV2Fixture()];
    server.generateFixturePresets.mockResolvedValueOnce({
      created: [
        {
          address: { family: "Beam", number: 1 },
          number: 1,
          name: "Dots",
          family: "Beam",
        },
      ],
    });

    render(<ParameterControls />);
    expect(server.generateFixturePresets).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Direct values and actions" }));
    fireEvent.click(screen.getByRole("button", { name: "Generate portable presets" }));

    expect(server.generateFixturePresets).toHaveBeenCalledWith(["fixture-1"]);
    expect(await screen.findByRole("status")).toHaveTextContent("Created 1 portable preset");
  });
});
