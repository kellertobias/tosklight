import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { GroupStrip } from "./GroupStrip";

const mocks = vi.hoisted(() => ({
  dispatch: vi.fn(),
  executeCommandLine: vi.fn(),
  selectionGesture: vi.fn(),
  selectGroup: vi.fn(),
  refresh: vi.fn(),
  state: { storeArmed: false },
  groups: [
    {
      id: "1",
      body: {
        name: "Shortcut Group",
        fixtures: ["fixture-1"],
      },
    },
  ],
}));

vi.mock("../../api/ServerContext", () => ({
  useServer: () => ({
    bootstrap: { active_show: { id: "show" } },
    groups: mocks.groups,
    selectedGroupId: null,
    executeCommandLine: mocks.executeCommandLine,
    selectionGesture: mocks.selectionGesture,
    selectGroup: mocks.selectGroup,
    refresh: mocks.refresh,
  }),
}));

vi.mock("../../state/AppContext", () => ({
  useApp: () => ({
    state: mocks.state,
    dispatch: mocks.dispatch,
  }),
}));

describe("GroupStrip command routing", () => {
  afterEach(() => cleanup());

  beforeEach(() => {
    mocks.dispatch.mockReset();
    mocks.executeCommandLine.mockReset().mockResolvedValue(true);
    mocks.selectionGesture.mockReset().mockResolvedValue(undefined);
    mocks.selectGroup.mockReset().mockResolvedValue(undefined);
    mocks.refresh.mockReset().mockResolvedValue(undefined);
    mocks.state.storeArmed = false;
    mocks.groups = [
      {
        id: "1",
        body: {
          name: "Shortcut Group",
          fixtures: ["fixture-1"],
        },
      },
    ];
  });

  it("selects shortcut groups through the shared surface gesture", () => {
    render(<GroupStrip />);
    fireEvent.click(screen.getByText("Shortcut Group").closest("button")!);
    expect(mocks.selectionGesture).toHaveBeenCalledWith({ type: "live_group", group_id: "1" });
  });

  it("records directly into stored empty shortcut groups", async () => {
    mocks.state.storeArmed = true;
    mocks.groups = [
      {
        id: "1",
        body: {
          name: "Stored Empty Shortcut",
          fixtures: [],
        },
      },
    ];
    render(<GroupStrip />);
    fireEvent.click(screen.getByText("Stored Empty Shortcut").closest("button")!);
    await waitFor(() => expect(mocks.executeCommandLine).toHaveBeenCalledWith("RECORD GROUP 1"));
    expect(screen.queryByRole("dialog", { name: "Record to Stored Empty Shortcut" })).toBeNull();
    expect(mocks.refresh).toHaveBeenCalledOnce();
    expect(mocks.dispatch).toHaveBeenCalledWith({ type: "SET_STORE_ARMED", value: false });
  });

  it("uses RECORD + GROUP when Merge is chosen for populated shortcut groups", async () => {
    mocks.state.storeArmed = true;
    render(<GroupStrip />);
    fireEvent.click(screen.getByText("Shortcut Group").closest("button")!);
    fireEvent.click(screen.getByRole("button", { name: "Merge" }));
    await waitFor(() => expect(mocks.executeCommandLine).toHaveBeenCalledWith("RECORD + GROUP 1"));
    expect(mocks.refresh).toHaveBeenCalledOnce();
    expect(mocks.dispatch).toHaveBeenCalledWith({ type: "SET_STORE_ARMED", value: false });
  });
});
