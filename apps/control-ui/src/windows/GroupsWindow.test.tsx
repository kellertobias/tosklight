import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { GroupsWindow } from "./GroupsWindow";

const mocks = vi.hoisted(() => ({
  dispatch: vi.fn(),
  executeCommandLine: vi.fn(),
  refresh: vi.fn(),
  state: { storeArmed: false, groupsReturnToStage: false },
  groups: [
    {
      id: "4",
      revision: 1,
      updated_at: "",
      body: {
        name: "Stored Empty",
        fixtures: [],
        programming: {},
        master: 1,
        playback_fader: 4,
        derived_from: null,
        frozen_from: null,
      },
    },
    {
      id: "5",
      revision: 1,
      updated_at: "",
      body: {
        name: "Stored Populated",
        fixtures: ["fixture-1"],
        programming: {},
        master: 1,
        playback_fader: 5,
        derived_from: null,
        frozen_from: null,
      },
    },
  ],
}));

vi.mock("../api/ServerContext", () => ({
  useServer: () => ({
    bootstrap: { active_show: { id: "show" } },
    groups: mocks.groups,
    patch: { fixtures: [], revision: 0 },
    selectedFixtures: [],
    selectedGroupId: null,
    executeCommandLine: mocks.executeCommandLine,
    refresh: mocks.refresh,
    setGroupMaster: vi.fn(),
    undoGroup: vi.fn(),
    refreshFrozenGroup: vi.fn(),
    detachDerivedGroup: vi.fn(),
  }),
}));

vi.mock("../state/AppContext", () => ({
  useApp: () => ({
    state: mocks.state,
    dispatch: mocks.dispatch,
  }),
}));

describe("GroupsWindow command routing", () => {
  afterEach(() => cleanup());

  beforeEach(() => {
    mocks.dispatch.mockReset();
    mocks.executeCommandLine.mockReset().mockResolvedValue(true);
    mocks.refresh.mockReset().mockResolvedValue(undefined);
    mocks.state.storeArmed = false;
    mocks.state.groupsReturnToStage = false;
  });

  it("selects a stored group through the command line", () => {
    render(<GroupsWindow />);
    fireEvent.click(screen.getByText("Stored Empty").closest("button")!);
    expect(mocks.executeCommandLine).toHaveBeenCalledWith("GROUP 4");
  });

  it("records directly into a stored empty group through RECORD GROUP", async () => {
    mocks.state.storeArmed = true;
    render(<GroupsWindow />);
    fireEvent.click(screen.getByText("Stored Empty").closest("button")!);
    await waitFor(() => expect(mocks.executeCommandLine).toHaveBeenCalledWith("RECORD GROUP 4"));
    expect(screen.queryByRole("dialog", { name: "Record to Stored Empty" })).toBeNull();
    expect(mocks.refresh).toHaveBeenCalledOnce();
    expect(mocks.dispatch).toHaveBeenCalledWith({ type: "SET_STORE_ARMED", value: false });
  });

  it("records empty pool cells through RECORD GROUP without a mode dialog", async () => {
    mocks.state.storeArmed = true;
    render(<GroupsWindow />);
    fireEvent.click(screen.getAllByText("Tap to record empty group")[0].closest("button")!);
    await waitFor(() => expect(mocks.executeCommandLine).toHaveBeenCalledWith("RECORD GROUP 1"));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(mocks.refresh).toHaveBeenCalledOnce();
  });

  it("uses RECORD + GROUP when Merge is chosen for a populated group", async () => {
    mocks.state.storeArmed = true;
    render(<GroupsWindow />);
    fireEvent.click(screen.getByText("Stored Populated").closest("button")!);
    fireEvent.click(screen.getByRole("button", { name: "Merge" }));
    await waitFor(() => expect(mocks.executeCommandLine).toHaveBeenCalledWith("RECORD + GROUP 5"));
    expect(mocks.refresh).toHaveBeenCalledOnce();
    expect(mocks.dispatch).toHaveBeenCalledWith({ type: "SET_STORE_ARMED", value: false });
  });
});
