import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { PlaybackDefinition } from "../api/types";
import { CuelistWindow } from "./CuelistWindow";

const mocks = vi.hoisted(() => ({
  dispatch: vi.fn(),
  executeCommandLine: vi.fn(),
  setCommandLine: vi.fn(),
  refresh: vi.fn(),
  state: { storeArmed: true, cueListSetArmed: false, cueListSetTarget: null as number | null },
  playbacks: { pool: [] as PlaybackDefinition[], active: [], pages: [], cue_lists: [], active_page: 1 },
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
    mocks.state.storeArmed = true;
    mocks.state.cueListSetArmed = false;
    mocks.state.cueListSetTarget = null;
    mocks.playbacks.pool = [];
  });

  it("renders empty numbered slots and records into the touched slot", async () => {
    render(<CuelistWindow compact cueListTab="pool" />);
    expect(screen.getAllByText("Tap to record Cuelist")).toHaveLength(1000);
    fireEvent.click(screen.getAllByText("Tap to record Cuelist")[0].closest("button")!);
    await waitFor(() => expect(mocks.executeCommandLine).toHaveBeenCalledWith("RECORD SET 1"));
    expect(mocks.setCommandLine).toHaveBeenCalledWith("");
    expect(mocks.refresh).toHaveBeenCalledOnce();
    expect(mocks.dispatch).toHaveBeenCalledWith({ type: "SET_STORE_ARMED", value: false });
  });

  it("selects an existing pool playback as the next Set assignment source", () => {
    mocks.state.storeArmed = false;
    mocks.state.cueListSetArmed = true;
    mocks.playbacks.pool = [{ number: 7, name: "Main sequence", target: { type: "cue_list", cue_list_id: "main" }, buttons: ["go", "go_minus", "flash"], fader: "master", go_activates: true, auto_off: true, xfade_millis: 0 }];
    render(<CuelistWindow compact cueListTab="pool" />);
    fireEvent.click(screen.getByText("Main sequence").closest("button")!);
    expect(mocks.dispatch).toHaveBeenCalledWith({ type: "SET_CUELIST_SET_TARGET", value: 7 });
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
