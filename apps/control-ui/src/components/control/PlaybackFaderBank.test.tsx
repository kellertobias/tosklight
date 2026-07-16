import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { PlaybackFaderBank } from "./PlaybackFaderBank";
import { Button } from "../common";

const mocks = vi.hoisted(() => ({
  dispatch: vi.fn(),
  executeCommandLine: vi.fn(),
  refresh: vi.fn(),
  poolPlaybackAction: vi.fn(),
  unassignPagePlayback: vi.fn(),
  state: {
    midiProfile: null,
    playbackColumns: 1,
    playbackRows: 1,
    playbackPage: 0,
    cueListSetTarget: 12 as number | null,
    cueListSetArmed: true,
    playbackSetArmed: false,
    shiftArmed: false,
  },
  playbacks: {
    active_page: 1,
    pages: [{ number: 1, name: "Main", slots: {} as Record<string, number> }],
    pool: [] as Array<Record<string, unknown>>,
    active: [],
    cue_lists: [],
    desk: { buttons: 3 },
  },
}));

vi.mock("../../api/ServerContext", () => ({
  useServer: () => ({
    bootstrap: { hardware_connected: false },
    playbacks: mocks.playbacks,
    groups: [],
    executeCommandLine: mocks.executeCommandLine,
    refresh: mocks.refresh,
    poolPlaybackAction: mocks.poolPlaybackAction,
    unassignPagePlayback: mocks.unassignPagePlayback,
  }),
}));

vi.mock("../../state/AppContext", () => ({
  useApp: () => ({
    state: mocks.state,
    dispatch: mocks.dispatch,
  }),
}));

vi.mock("./VerticalTouchFader", () => ({ VerticalTouchFader: ({ actions = [] }: { actions?: Array<{ id: string; label: string; onClick?: (event: React.MouseEvent<HTMLButtonElement>) => void }> }) => <div>Fader{actions.map((action) => <Button key={action.id} onClick={action.onClick}>{action.label}</Button>)}</div> }));

afterEach(cleanup);

describe("PlaybackFaderBank Set assignment", () => {
  beforeEach(() => {
    mocks.dispatch.mockReset();
    mocks.executeCommandLine.mockReset().mockResolvedValue(true);
    mocks.refresh.mockReset().mockResolvedValue(undefined);
    mocks.poolPlaybackAction.mockReset();
    mocks.unassignPagePlayback.mockReset().mockResolvedValue(true);
    Object.assign(mocks.state, { cueListSetTarget: 12, cueListSetArmed: true, playbackSetArmed: false, shiftArmed: false });
    mocks.playbacks.pages[0].slots = {};
    mocks.playbacks.pool = [];
  });

  it("assigns the selected pool playback to the touched page slot", async () => {
    render(<PlaybackFaderBank count={1} />);
    fireEvent.click(screen.getByRole("button", { name: "Assign Cuelist 12 to page 1 playback 1" }));
    await waitFor(() => expect(mocks.executeCommandLine).toHaveBeenCalledWith("SET 12 AT 1.1"));
    expect(mocks.refresh).toHaveBeenCalledOnce();
    expect(mocks.dispatch).toHaveBeenCalledWith({ type: "SET_CUELIST_SET_ARMED", value: false });
  });

  const assignPlayback = () => {
    mocks.playbacks.pages[0].slots = { "1": 7 };
    mocks.playbacks.pool = [{ number: 7, name: "Front Wash", target: { type: "cue_list", cue_list_id: "front" }, buttons: ["go", "go_minus", "flash"], fader: "master", go_activates: true, auto_off: true, xfade_millis: 0 }];
    Object.assign(mocks.state, { cueListSetTarget: null, cueListSetArmed: false });
  };

  it("opens the large configuration modal when SET is armed and the playback is clicked", () => {
    assignPlayback();
    mocks.state.playbackSetArmed = true;
    render(<PlaybackFaderBank count={1}/>);
    fireEvent.click(screen.getByRole("button", { name: "Configure page 1 playback 1" }));
    expect(screen.getByRole("dialog", { name: "Playback Configuration" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Clear Playback" })).toBeInTheDocument();
  });

  it("opens configuration when SHIFT is followed by the first playback button", () => {
    assignPlayback();
    mocks.state.shiftArmed = true;
    render(<PlaybackFaderBank count={1}/>);
    fireEvent.click(screen.getByRole("button", { name: "GO" }));
    expect(screen.getByRole("dialog", { name: "Playback Configuration" })).toBeInTheDocument();
    expect(mocks.poolPlaybackAction).not.toHaveBeenCalled();
  });

  it("recognizes the marked click produced by a playback right-click", () => {
    assignPlayback();
    const { container } = render(<PlaybackFaderBank count={1}/>);
    const playback = container.querySelector("article")!;
    const click = new MouseEvent("click", { bubbles: true, cancelable: true });
    Object.defineProperty(click, "lightSetShortcut", { value: true });
    fireEvent(playback, click);
    expect(screen.getByRole("dialog", { name: "Playback Configuration" })).toBeInTheDocument();
  });

  it("unassigns the page slot and closes the configuration modal", async () => {
    assignPlayback();
    mocks.state.playbackSetArmed = true;
    render(<PlaybackFaderBank count={1}/>);
    fireEvent.click(screen.getByRole("button", { name: "Configure page 1 playback 1" }));
    fireEvent.click(screen.getByRole("button", { name: "Clear Playback" }));
    fireEvent.click(screen.getByRole("button", { name: "Confirm Clear Playback" }));
    await waitFor(() => expect(mocks.unassignPagePlayback).toHaveBeenCalledWith(1, 1));
    await waitFor(() => expect(screen.queryByRole("dialog", { name: "Playback Configuration" })).not.toBeInTheDocument());
  });
});
