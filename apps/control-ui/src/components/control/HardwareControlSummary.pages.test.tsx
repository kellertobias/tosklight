import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { HardwareControlSummary } from "./HardwareControlSummary";

const dispatch = vi.fn();
const state = { playbackPage: 0, playbackSetArmed: false };
let playbackDesk = { active_page: 1 };
const server = {
  playbacks: { active_page: 1, pages: [{ number: 1, name: "Main", slots: {} }] },
  configuration: { speed_groups_bpm: [120, 90, 60, 30, 15], programmer_fade_millis: 3_000, sequence_master_fade_millis: 3_000 },
  savePlaybackPage: vi.fn(async () => true),
  setPlaybackPage: vi.fn(),
  setControlTiming: vi.fn(),
  highlightError: null,
  dismissHighlightError: vi.fn(),
};

vi.mock("../../state/AppContext", () => ({ useApp: () => ({ state, dispatch: (action: { type: string; value?: boolean }) => {
  if (action.type === "SET_PLAYBACK_SET_ARMED") state.playbackSetArmed = Boolean(action.value);
  dispatch(action);
} }) }));
vi.mock("../../api/ServerContext", () => ({ useServer: () => server }));
vi.mock("../../features/playbackRuntime/PlaybackRuntimeView", () => ({
  usePlaybackDeskView: () => playbackDesk,
}));

afterEach(() => { cleanup(); state.playbackSetArmed = false; playbackDesk = { active_page: 1 }; vi.clearAllMocks(); });

describe("HardwareControlSummary playback pages", () => {
  it("offers Add new page from the hardware-connected page menu", async () => {
    render(<HardwareControlSummary/>);
    fireEvent.click(screen.getByRole("button", { name: "Page 1" }));
    fireEvent.click(within(screen.getByRole("dialog", { name: "Playback pages" })).getByRole("button", { name: "Add new page" }));
    await waitFor(() => expect(server.savePlaybackPage).toHaveBeenCalledWith({ number: 2, name: "Page 2", slots: {} }));
    expect(server.setPlaybackPage).toHaveBeenCalledWith(2);
  });

  it("uses SET then Page to rename instead of opening the page menu", () => {
    state.playbackSetArmed = true;
    render(<HardwareControlSummary/>);
    fireEvent.click(screen.getByRole("button", { name: "Page 1" }));
    expect(screen.getByRole("dialog", { name: "Rename playback page 1" })).toBeInTheDocument();
    expect(screen.queryByRole("dialog", { name: "Playback pages" })).not.toBeInTheDocument();
    expect(dispatch).toHaveBeenCalledWith({ type: "SET_PLAYBACK_SET_ARMED", value: false });
  });

  it("renders the scoped Playback desk page instead of stale bootstrap state", () => {
    playbackDesk = { active_page: 2 };
    render(<HardwareControlSummary/>);

    expect(screen.getByRole("button", { name: "Page 2" })).toBeVisible();
    expect(screen.queryByRole("button", { name: "Page 1" })).not.toBeInTheDocument();
  });
});
