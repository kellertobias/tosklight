import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { PaneSettingsModal } from "../components/modals/PaneSettingsModal";
import { VirtualPlaybacksWindow } from "./VirtualPlaybacksWindow";

const mocks = vi.hoisted(() => ({
  dispatch: vi.fn(), poolPlaybackAction: vi.fn(), savePlaybackSlot: vi.fn(), clearPlaybackSlot: vi.fn(), readVirtualPlaybackExclusionZones: vi.fn(), saveVirtualPlaybackExclusionZones: vi.fn(), error: null as string | null,
  state: {
    activeDeskId: "desk-1", paneSettingsId: null as string | null, playbackPage: 0, playbackSetArmed: false, cueListSetArmed: false, cueListSetTarget: null as number | null, shiftArmed: false,
    desks: [{ id: "desk-1", name: "Desk 1", panes: [{ id: "virtual-1", kind: "virtual_playbacks", title: "Virtual Playbacks", x: 1, y: 1, width: 6, height: 6, virtualPlaybackRows: 1, virtualPlaybackColumns: 2, virtualPlaybackCells: [{ playbackNumber: 999, action: "toggle" }], virtualPlaybackExclusionZones: [] as Array<{ id: string; name: string; slots: number[] }> }] }],
  },
  playbacks: {
    active_page: 1, pages: [{ number: 1, name: "Main", slots: { "1": 7 } as Record<string, number> }],
    pool: [{ number: 7, name: "Front Wash", target: { type: "cue_list", cue_list_id: "cue-1" }, buttons: ["toggle", "none", "none"], button_count: 1, fader: "master", has_fader: false, go_activates: true, auto_off: true, xfade_millis: 0, color: "#8b5cf6", flash_release: "release_all", protect_from_swap: false }],
    active: [] as Array<Record<string, any>>, cue_lists: [{ id: "cue-1", name: "Front sequence", cues: [] }], desk: { buttons: 3 },
  },
}));

vi.mock("../state/AppContext", () => ({ useApp: () => ({ state: mocks.state, dispatch: mocks.dispatch }) }));
vi.mock("../api/ServerContext", () => ({ useServer: () => ({ playbacks: mocks.playbacks, groups: [], poolPlaybackAction: mocks.poolPlaybackAction, savePlaybackSlot: mocks.savePlaybackSlot, clearPlaybackSlot: mocks.clearPlaybackSlot, readVirtualPlaybackExclusionZones: mocks.readVirtualPlaybackExclusionZones, saveVirtualPlaybackExclusionZones: mocks.saveVirtualPlaybackExclusionZones, error: mocks.error }) }));

afterEach(cleanup);
beforeEach(() => {
  mocks.dispatch.mockReset(); mocks.poolPlaybackAction.mockReset().mockResolvedValue(undefined); mocks.savePlaybackSlot.mockReset().mockResolvedValue(true); mocks.clearPlaybackSlot.mockReset().mockResolvedValue(true); mocks.readVirtualPlaybackExclusionZones.mockReset().mockResolvedValue({ show_id: "show-1", desk_id: "desk-1", surfaces: {} }); mocks.saveVirtualPlaybackExclusionZones.mockReset().mockResolvedValue(true); mocks.error = null;
  Object.assign(mocks.state, { paneSettingsId: null, playbackSetArmed: false, cueListSetArmed: false, cueListSetTarget: null, shiftArmed: false });
  mocks.playbacks.pages[0].slots = { "1": 7 }; mocks.playbacks.active = []; mocks.state.desks[0].panes[0].virtualPlaybackExclusionZones = [];
});

describe("VirtualPlaybacksWindow", () => {
  it("projects the active page slots and ignores the removed local per-cell assignment model", () => {
    render(<VirtualPlaybacksWindow paneId="virtual-1"/>);
    const cell = screen.getByRole("button", { name: "Virtual playback page 1 cell 1 Front Wash" });
    expect(cell).toHaveTextContent("TOGGLE");
    fireEvent.click(cell);
    expect(mocks.poolPlaybackAction).toHaveBeenCalledWith(7, "button", { button: 1, pressed: true, surface: "virtual" });
  });

  it("SET plus an empty cell opens the same one-button faderless modal without assigning", () => {
    mocks.state.playbackSetArmed = true;
    render(<VirtualPlaybacksWindow paneId="virtual-1"/>);
    fireEvent.click(screen.getByRole("button", { name: "Virtual playback page 1 cell 2 empty" }));
    const modal = screen.getByRole("dialog", { name: "Playback Configuration" });
    expect(modal).toHaveAttribute("data-page", "1"); expect(modal).toHaveAttribute("data-slot", "2"); expect(modal).toHaveAttribute("data-topology", "1 button · faderless");
    expect(screen.getByText("Presentation", { selector: "label", exact: true }).closest(".ui-form-field")?.querySelector(".ui-select-trigger")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(mocks.savePlaybackSlot).not.toHaveBeenCalled(); expect(mocks.poolPlaybackAction).not.toHaveBeenCalled();
  });

  it("assigns a selected Cuelist source atomically as a one-button faderless playback", async () => {
    mocks.state.cueListSetArmed = true; mocks.state.cueListSetTarget = 7; mocks.playbacks.pages[0].slots = {};
    render(<VirtualPlaybacksWindow paneId="virtual-1"/>);
    fireEvent.click(screen.getByRole("button", { name: "Virtual playback page 1 cell 1 empty" }));
    await waitFor(() => expect(mocks.savePlaybackSlot).toHaveBeenCalledWith(1, 1, expect.objectContaining({ number: 0, target: { type: "cue_list", cue_list_id: "cue-1" }, buttons: ["toggle", "none", "none"], button_count: 1, has_fader: false })));
    expect(mocks.dispatch).toHaveBeenCalledWith({ type: "SET_CUELIST_SET_ARMED", value: false });
  });

  it("exposes Set Source and Add Target as normal assignment entry points", () => {
    render(<VirtualPlaybacksWindow paneId="virtual-1"/>);
    fireEvent.click(screen.getByRole("button", { name: "Set Source" }));
    expect(mocks.dispatch).toHaveBeenCalledWith({ type: "SET_CUELIST_SET_TARGET", value: null });
    expect(mocks.dispatch).toHaveBeenCalledWith({ type: "SET_CUELIST_SET_ARMED", value: true });
    mocks.dispatch.mockClear(); fireEvent.click(screen.getByRole("button", { name: "Add Target" }));
    expect(mocks.dispatch).toHaveBeenCalledWith({ type: "SET_CUELIST_SET_ARMED", value: true });
  });

  it("Shift-selects cells without operating them and creates a named persisted exclusion zone", async () => {
    mocks.state.shiftArmed = true;
    render(<VirtualPlaybacksWindow paneId="virtual-1"/>);
    fireEvent.click(screen.getByRole("button", { name: "Virtual playback page 1 cell 1 Front Wash" }));
    fireEvent.click(screen.getByRole("button", { name: "Virtual playback page 1 cell 2 empty" }));
    expect(mocks.poolPlaybackAction).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Virtual playback page 1 cell 1 Front Wash" })).toHaveAttribute("aria-pressed", "true");
    fireEvent.click(screen.getByRole("button", { name: "Create Exclusion Zone" }));
    fireEvent.change(screen.getByLabelText("Zone name"), { target: { value: "Front alternates" } });
    fireEvent.click(screen.getByRole("button", { name: "Create zone" }));
    await waitFor(() => expect(mocks.saveVirtualPlaybackExclusionZones).toHaveBeenCalledWith("virtual-1", [expect.objectContaining({ name: "Front alternates", slots: [1, 2] })]));
    expect(mocks.dispatch).toHaveBeenCalledWith({ type: "SET_VIRTUAL_PLAYBACK_EXCLUSION_ZONES", id: "virtual-1", zones: [expect.objectContaining({ name: "Front alternates", slots: [1, 2] })] });
    expect(mocks.dispatch).toHaveBeenCalledWith({ type: "SET_SHIFT_ARMED", value: false });
  });
});

describe("Virtual Playback Pane Settings", () => {
  it("contains Rows, Columns, and the named exclusion-zone list with retained hidden members", () => {
    mocks.state.paneSettingsId = "virtual-1";
    mocks.state.desks[0].panes[0].virtualPlaybackExclusionZones = [{ id: "zone-1", name: "Front alternates", slots: [1, 2, 4] }];
    render(<PaneSettingsModal/>);
    fireEvent.click(screen.getByRole("tab", { name: "Virtual Playbacks" }));
    expect(screen.getByLabelText("Rows")).toBeInTheDocument(); expect(screen.getByLabelText("Columns")).toBeInTheDocument();
    expect(screen.queryByText(/Cell 1 Cuelist/)).not.toBeInTheDocument(); expect(screen.queryByText(/Cell 1 action/)).not.toBeInTheDocument();
    expect(screen.getByText(/Set Source/)).toBeInTheDocument(); expect(screen.getByText(/Add Target/)).toBeInTheDocument();
    expect(screen.getByRole("region", { name: "Playback Exclusion Zones" })).toBeInTheDocument();
    expect(screen.getByText("1 hidden grid cell is retained:")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Front alternates hidden cell 4" })).toBeInTheDocument();
  });

  it("renames, edits, and deletes zones through authoritative persistence", async () => {
    mocks.state.paneSettingsId = "virtual-1";
    mocks.state.desks[0].panes[0].virtualPlaybackExclusionZones = [{ id: "zone-1", name: "Front alternates", slots: [1, 2, 4] }];
    render(<PaneSettingsModal/>);
    fireEvent.click(screen.getByRole("tab", { name: "Virtual Playbacks" }));
    fireEvent.change(screen.getByLabelText("Name for Front alternates"), { target: { value: "Front choice" } });
    fireEvent.click(screen.getByRole("button", { name: "Save name" }));
    await waitFor(() => expect(mocks.saveVirtualPlaybackExclusionZones).toHaveBeenCalledWith("virtual-1", [{ id: "zone-1", name: "Front choice", slots: [1, 2, 4] }]));
    mocks.saveVirtualPlaybackExclusionZones.mockClear();
    fireEvent.click(screen.getByRole("button", { name: "Front alternates hidden cell 4" }));
    await waitFor(() => expect(mocks.saveVirtualPlaybackExclusionZones).toHaveBeenCalledWith("virtual-1", [{ id: "zone-1", name: "Front alternates", slots: [1, 2] }]));
    mocks.saveVirtualPlaybackExclusionZones.mockClear();
    fireEvent.click(screen.getByRole("button", { name: "Delete zone" }));
    await waitFor(() => expect(mocks.saveVirtualPlaybackExclusionZones).toHaveBeenCalledWith("virtual-1", []));
  });
});
