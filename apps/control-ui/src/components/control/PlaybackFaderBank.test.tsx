import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { PlaybackFaderBank } from "./PlaybackFaderBank";

const mocks = vi.hoisted(() => ({
  dispatch: vi.fn(),
  executeCommandLine: vi.fn(),
  refresh: vi.fn(),
}));

vi.mock("../../api/ServerContext", () => ({
  useServer: () => ({
    bootstrap: { hardware_connected: false },
    playbacks: { active_page: 1, pages: [{ number: 1, name: "Main", slots: {} }], pool: [], active: [], cue_lists: [], desk: { buttons: 3 } },
    groups: [],
    executeCommandLine: mocks.executeCommandLine,
    refresh: mocks.refresh,
    poolPlaybackAction: vi.fn(),
  }),
}));

vi.mock("../../state/AppContext", () => ({
  useApp: () => ({
    state: { midiProfile: null, playbackColumns: 1, playbackRows: 1, playbackPage: 0, cueListSetTarget: 12 },
    dispatch: mocks.dispatch,
  }),
}));

vi.mock("./VerticalTouchFader", () => ({ VerticalTouchFader: () => <div>Fader</div> }));

describe("PlaybackFaderBank Set assignment", () => {
  beforeEach(() => {
    mocks.dispatch.mockReset();
    mocks.executeCommandLine.mockReset().mockResolvedValue(true);
    mocks.refresh.mockReset().mockResolvedValue(undefined);
  });

  it("assigns the selected pool playback to the touched page slot", async () => {
    render(<PlaybackFaderBank count={1} />);
    fireEvent.click(screen.getByRole("button", { name: "Assign Cuelist 12 to page 1 playback 1" }));
    await waitFor(() => expect(mocks.executeCommandLine).toHaveBeenCalledWith("SET 12 AT 1.1"));
    expect(mocks.refresh).toHaveBeenCalledOnce();
    expect(mocks.dispatch).toHaveBeenCalledWith({ type: "SET_CUELIST_SET_ARMED", value: false });
  });
});
