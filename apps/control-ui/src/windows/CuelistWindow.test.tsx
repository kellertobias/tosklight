import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { CueList, PlaybackDefinition } from "../api/types";
import { CuelistWindow } from "./CuelistWindow";

const mocks = vi.hoisted(() => ({
  dispatch: vi.fn(),
  executeCommandLine: vi.fn(),
  setCommandLine: vi.fn(),
  refresh: vi.fn(),
  saveCueList: vi.fn(),
  state: {
    storeArmed: true,
    cueListSetArmed: false,
    cueListSetTarget: null as number | null,
  },
  playbacks: {
    pool: [] as PlaybackDefinition[],
    active: [],
    pages: [],
    cue_lists: [] as CueList[],
    active_page: 1,
  },
  cueObjects: [] as Array<Record<string, unknown>>,
}));

vi.mock("../api/ServerContext", () => ({
  useServer: () => ({
    playbacks: mocks.playbacks,
    patch: { fixtures: [], revision: 0 },
    stageLayout: null,
    groups: [],
    readVisualization: vi.fn(),
    executeCommandLine: mocks.executeCommandLine,
    setCommandLine: mocks.setCommandLine,
    refresh: mocks.refresh,
    cueObjects: mocks.cueObjects,
    saveCueList: mocks.saveCueList,
  }),
}));

vi.mock("../state/AppContext", () => ({
  useApp: () => ({
    state: mocks.state,
    dispatch: mocks.dispatch,
  }),
}));

vi.mock("./stage3dScene", () => ({
  cueVisualization: vi.fn(),
  migrateStagePosition: vi.fn(),
  renderStageThumbnail: vi.fn(),
}));

describe("CuelistWindow pool recording", () => {
  beforeEach(() => {
    mocks.dispatch.mockReset();
    mocks.executeCommandLine.mockReset().mockResolvedValue(true);
    mocks.setCommandLine.mockReset();
    mocks.refresh.mockReset().mockResolvedValue(undefined);
    mocks.saveCueList.mockReset().mockResolvedValue(true);
    mocks.state.storeArmed = true;
    mocks.state.cueListSetArmed = false;
    mocks.state.cueListSetTarget = null;
    mocks.playbacks.pool = [];
    mocks.playbacks.cue_lists = [];
    mocks.cueObjects = [];
  });

  it("keeps Cue rows selection-only and exposes the five-column editor", () => {
    mocks.state.storeArmed = false;
    mocks.playbacks.pool = [
      {
        number: 1,
        name: "Main",
        target: { type: "cue_list", cue_list_id: "main" },
        buttons: ["go", "go_minus", "flash"],
        fader: "master",
        go_activates: true,
        auto_off: true,
        xfade_millis: 0,
      },
    ];
    mocks.playbacks.cue_lists = [
      {
        id: "main",
        name: "Main",
        priority: 10,
        mode: "sequence",
        looped: false,
        cues: [
          {
            number: 1,
            name: "Opening",
            fade_millis: 1000,
            delay_millis: 0,
            trigger: { type: "manual" },
            changes: [],
          },
        ],
      },
    ];
    render(<CuelistWindow />);
    fireEvent.click(screen.getByText("Main").closest("button")!);
    expect(screen.getAllByRole("columnheader").map((cell) => cell.textContent)).toEqual(["Preview image", "Cue number", "Cue name", "Trigger", "Fade time"]);
    fireEvent.click(screen.getByText("Opening"));
    expect(screen.queryByRole("button", { name: "GO −" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "TOGGLE" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "OFF" })).not.toBeInTheDocument();
    expect(screen.getByLabelText("Title")).toHaveValue("Opening");
  });

  it("renders empty numbered slots and records into the touched slot", async () => {
    render(<CuelistWindow compact cueListTab="pool" />);
    expect(screen.getAllByText("Tap to record Cuelist")).toHaveLength(1000);
    fireEvent.click(screen.getAllByText("Tap to record Cuelist")[0].closest("button")!);
    await waitFor(() => expect(mocks.executeCommandLine).toHaveBeenCalledWith("RECORD SET 1"));
    expect(mocks.setCommandLine).toHaveBeenCalledWith("");
    expect(mocks.refresh).toHaveBeenCalledOnce();
    expect(mocks.dispatch).toHaveBeenCalledWith({
      type: "SET_STORE_ARMED",
      value: false,
    });
  });

  it("selects an existing pool playback as the next Set assignment source", () => {
    mocks.state.storeArmed = false;
    mocks.state.cueListSetArmed = true;
    mocks.playbacks.pool = [
      {
        number: 7,
        name: "Main sequence",
        target: { type: "cue_list", cue_list_id: "main" },
        buttons: ["go", "go_minus", "flash"],
        fader: "master",
        go_activates: true,
        auto_off: true,
        xfade_millis: 0,
      },
    ];
    render(<CuelistWindow compact cueListTab="pool" />);
    fireEvent.click(screen.getByText("Main sequence").closest("button")!);
    expect(mocks.dispatch).toHaveBeenCalledWith({
      type: "SET_CUELIST_SET_TARGET",
      value: 7,
    });
  });

  it("shows the Set workflow in the header's secondary amber status line", () => {
    mocks.state.storeArmed = false;
    mocks.state.cueListSetArmed = true;
    const { container } = render(<CuelistWindow />);
    const status = container.querySelector(".cuelist-workflow-status")!;
    expect(status).toHaveTextContent("Select a Cuelist, then touch the playback fader where it should be assigned.");
    expect(status).toHaveClass("cuelist-workflow-status");
    expect(status.closest("small")).toBe(container.querySelector(".ui-window-info small"));
    expect(container.querySelector(".pool-message")).toBeNull();
  });
});
