import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { GroupsWindow } from "./GroupsWindow";

const mocks = vi.hoisted(() => ({
  dispatch: vi.fn(),
  executeCommandLine: vi.fn(),
  selectionGesture: vi.fn(),
  refresh: vi.fn(),
  refreshGroup: vi.fn(),
  resetCommandLine: vi.fn(),
  updateGroup: vi.fn(),
  commandLine: "",
  state: { storeArmed: false, groupsReturnToStage: false },
  groups: [
    {
      id: "4",
      revision: 1,
      updated_at: "",
      body: {
        name: "Stored Empty",
        color: undefined as string | undefined,
        icon: undefined as string | undefined,
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
        color: undefined as string | undefined,
        icon: undefined as string | undefined,
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
    selectionGesture: mocks.selectionGesture,
    refresh: mocks.refresh,
    refreshGroup: mocks.refreshGroup,
    resetCommandLine: mocks.resetCommandLine,
    updateGroup: mocks.updateGroup,
    commandLine: mocks.commandLine,
    setGroupMaster: vi.fn(),
    undoGroup: vi.fn(),
    refreshFrozenGroup: vi.fn(),
    detachDerivedGroup: vi.fn(),
  }),
}));
vi.mock("../features/server/useShowObjectsState", () => ({
  useGroups: () => mocks.groups,
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
    mocks.selectionGesture.mockReset().mockResolvedValue(undefined);
    mocks.refresh.mockReset().mockResolvedValue(undefined);
    mocks.refreshGroup.mockReset().mockResolvedValue(true);
    mocks.resetCommandLine.mockReset();
    mocks.updateGroup.mockReset().mockResolvedValue(true);
    mocks.commandLine = "";
    mocks.state.storeArmed = false;
    mocks.state.groupsReturnToStage = false;
    mocks.groups[0].body.color = undefined;
    mocks.groups[0].body.icon = undefined;
  });

  it("selects a stored group through the shared surface gesture", () => {
    render(<GroupsWindow />);
    fireEvent.click(screen.getByText("Stored Empty").closest("button")!);
    expect(mocks.selectionGesture).toHaveBeenCalledWith({ type: "live_group", group_id: "4" });
  });

  it("records directly into a stored empty group through RECORD GROUP", async () => {
    mocks.state.storeArmed = true;
    render(<GroupsWindow />);
    fireEvent.click(screen.getByText("Stored Empty").closest("button")!);
    await waitFor(() => expect(mocks.executeCommandLine).toHaveBeenCalledWith("RECORD GROUP 4"));
    expect(screen.queryByRole("dialog", { name: "Record to Stored Empty" })).toBeNull();
    expect(mocks.refreshGroup).toHaveBeenCalledWith("4");
    expect(mocks.refresh).not.toHaveBeenCalled();
    expect(mocks.dispatch).toHaveBeenCalledWith({ type: "SET_STORE_ARMED", value: false });
  });

  it("records empty pool cells through RECORD GROUP without a mode dialog", async () => {
    mocks.state.storeArmed = true;
    render(<GroupsWindow />);
    fireEvent.click(screen.getAllByText("Tap to record empty group")[0].closest("button")!);
    await waitFor(() => expect(mocks.executeCommandLine).toHaveBeenCalledWith("RECORD GROUP 1"));
    expect(screen.queryByRole("dialog")).toBeNull();
    expect(mocks.refreshGroup).toHaveBeenCalledWith("1");
    expect(mocks.refresh).not.toHaveBeenCalled();
  });

  it("uses RECORD + GROUP when Merge is chosen for a populated group", async () => {
    mocks.state.storeArmed = true;
    render(<GroupsWindow />);
    fireEvent.click(screen.getByText("Stored Populated").closest("button")!);
    fireEvent.click(screen.getByRole("button", { name: "Merge" }));
    await waitFor(() => expect(mocks.executeCommandLine).toHaveBeenCalledWith("RECORD + GROUP 5"));
    expect(mocks.refreshGroup).toHaveBeenCalledWith("5");
    expect(mocks.refresh).not.toHaveBeenCalled();
    expect(mocks.dispatch).toHaveBeenCalledWith({ type: "SET_STORE_ARMED", value: false });
  });

  it("opens and saves group properties when SET is armed before tapping the tile", async () => {
    mocks.commandLine = "SET ";
    render(<GroupsWindow />);
    fireEvent.click(screen.getByText("Stored Empty").closest("button")!);
    expect(mocks.resetCommandLine).toHaveBeenCalledOnce();
    expect(screen.getByRole("dialog", { name: "Group properties" })).toBeInTheDocument();
    fireEvent.change(screen.getByLabelText("Group name"), { target: { value: "Copy Center Spot" } });
    fireEvent.click(screen.getByRole("button", { name: /#718596/ }));
    fireEvent.click(screen.getByRole("option", { name: "Use color #1bd6ec" }));
    fireEvent.click(screen.getByRole("button", { name: /Choose icon/ }));
    fireEvent.click(await screen.findByRole("button", { name: "Use ★" }));
    fireEvent.click(screen.getByRole("button", { name: "Save group" }));
    await waitFor(() => expect(mocks.updateGroup).toHaveBeenCalledWith("4", {
      name: "Copy Center Spot",
      color: "#1bd6ec",
      icon: "★",
    }));
  });

  it("opens the same populated properties modal for a desk-routed SET command", () => {
    mocks.groups[0].body.color = "#d76cff";
    mocks.groups[0].body.icon = "●";
    render(<GroupsWindow />);
    act(() => window.dispatchEvent(new CustomEvent("light:group-configuration", { detail: "4" })));
    expect(screen.getByRole("dialog", { name: "Group properties" })).toBeInTheDocument();
    expect(screen.getByLabelText("Group name")).toHaveValue("Stored Empty");
    expect(screen.getByRole("button", { name: /#D76CFF/ })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /Choose icon/ })).toHaveTextContent("●");
  });
});
