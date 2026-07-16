import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { SystemControlsModal } from "./SystemControlsModal";

const dispatch = vi.fn();
const playbackAction = vi.fn().mockResolvedValue(undefined);
const clearProgrammer = vi.fn().mockResolvedValue(undefined);
const server = {
  readVisualization: vi.fn().mockResolvedValue({ grand_master: 1, blackout: false }),
  setMaster: vi.fn(),
  setProgrammer: vi.fn(),
  selectedFixtures: [],
  patch: { fixtures: [] },
  session: { user: { id: "operator", name: "Operator" } },
  bootstrap: {
    active_programmers: [{ session_id: "session-1", user_id: "operator", selected: ["fixture-1"], values: [{}], group_values: { front: { intensity: {} } }, connected: true }],
  },
  playbacks: {
    active: [
      { playback_number: 12, cue_list_id: "cue-list-1", cue_index: 0, paused: false, master: 0.75, flash: false },
      { playback_number: null, cue_list_id: "cue-list-2", cue_index: 0, paused: true, master: 1, flash: false },
    ],
    pool: [{ number: 12, name: "Main playback" }],
    cue_lists: [
      { id: "cue-list-1", name: "Main Cuelist", cues: [{ number: 1, phasers: [{}] }] },
      { id: "cue-list-2", name: "Virtual Cuelist", cues: [{ number: 3, phasers: [] }] },
    ],
  },
  playbackAction,
  clearProgrammer,
  preloadAction: vi.fn().mockResolvedValue(undefined),
};

vi.mock("../../api/ServerContext", () => ({ useServer: () => server }));
vi.mock("../../state/AppContext", () => ({ useApp: () => ({ state: { systemControlsOpen: true }, dispatch }) }));

afterEach(() => { cleanup(); vi.clearAllMocks(); });

describe("SystemControlsModal", () => {
  it("shows every running source and stops each one from the modal", () => {
    render(<SystemControlsModal/>);

    expect(screen.getByText("Main playback")).toBeInTheDocument();
    expect(screen.getByText("Virtual Cuelist")).toBeInTheDocument();
    expect(screen.getByText("Operator · Current user")).toBeInTheDocument();
    expect(screen.getByText("Main Cuelist · Dynamic 1")).toBeInTheDocument();
    expect(screen.getByText("1 fixtures · 2 values · Connected")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Stop Playback Main playback" }));
    fireEvent.click(screen.getByRole("button", { name: "Stop Virtual playback Virtual Cuelist" }));
    fireEvent.click(screen.getByRole("button", { name: "Clear programmer operator" }));

    expect(playbackAction).toHaveBeenCalledWith("cue-list-1", "release");
    expect(playbackAction).toHaveBeenCalledWith("cue-list-2", "release");
    expect(clearProgrammer).toHaveBeenCalledWith("session-1");
  });

  it("stops all playback and programmer sources together", async () => {
    render(<SystemControlsModal/>);
    fireEvent.click(screen.getByRole("button", { name: "Stop everything" }));

    await waitFor(() => expect(playbackAction).toHaveBeenCalledTimes(2));
    expect(clearProgrammer).toHaveBeenCalledWith("session-1");
    expect(server.preloadAction).toHaveBeenCalledWith("release");
    expect(dispatch).toHaveBeenCalledWith({ type: "RELEASE_PRELOAD" });
  });
});
