import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PlaybackDefinition } from "../../api/types";
import { normalizePlaybackTopology, PlaybackConfigurationModal, withFunctionDefaults } from "./PlaybackConfigurationModal";

const mocks = vi.hoisted(() => ({
  savePlaybackSlot: vi.fn(), clearPlaybackSlot: vi.fn(), error: null as string | null,
  playbacks: { desk: { buttons: 3 }, cue_lists: [{ id: "cue-1", name: "Main sequence" }, { id: "cue-2", name: "Encore" }] },
  groups: [{ id: "group-1", body: { name: "Front Wash" } }],
}));

vi.mock("../../api/ServerContext", () => ({ useServer: () => ({ ...mocks, playbacks: mocks.playbacks, groups: mocks.groups }) }));

const base: PlaybackDefinition = {
  number: 7, name: "Configured Playback", target: { type: "cue_list", cue_list_id: "cue-1" }, buttons: ["go_minus", "go", "flash"], button_count: 3,
  fader: "master", has_fader: true, go_activates: true, auto_off: true, xfade_millis: 0, color: "#20c997", flash_release: "release_all", protect_from_swap: false,
};

afterEach(cleanup);
beforeEach(() => { mocks.savePlaybackSlot.mockReset().mockResolvedValue(true); mocks.clearPlaybackSlot.mockReset().mockResolvedValue(true); mocks.error = null; });

function show(playback: PlaybackDefinition = base, props: { empty?: boolean; virtual?: boolean } = {}) {
  const close = vi.fn(); render(<PlaybackConfigurationModal playback={playback} page={2} slot={4} onClose={close} {...props}/>); return close;
}
function selectTrigger(label: string) { return screen.getByText(label, { selector: "label", exact: true }).closest(".ui-form-field")!.querySelector(".ui-select-trigger") as HTMLButtonElement; }
function choose(label: string, option: string) { fireEvent.click(selectTrigger(label)); fireEvent.click(screen.getByRole("option", { name: option })); }

describe("PlaybackConfigurationModal", () => {
  it("shows the exact default Cuelist layout and the compact 16-color palette", () => {
    show(); fireEvent.click(screen.getByRole("button", { name: "Playback Layout" }));
    expect(selectTrigger("Top button")).toHaveTextContent("Go minus");
    expect(selectTrigger("Middle button")).toHaveTextContent("Go plus");
    expect(selectTrigger("Bottom button")).toHaveTextContent("Flash");
    expect(selectTrigger("Fader")).toHaveTextContent("Master");
    expect(screen.getAllByRole("button", { name: /^Playback color #/ })).toHaveLength(16);
  });

  it("resets incompatible mappings when the assignment family changes", async () => {
    show({ ...base, buttons: ["swap", "select_contents", "fast_forward"], fader: "x_fade" });
    choose("Function", "Group Master");
    fireEvent.click(screen.getByRole("button", { name: "Playback Layout" }));
    expect(selectTrigger("Top button")).toHaveTextContent("Select");
    expect(selectTrigger("Middle button")).toHaveTextContent("Flash");
    expect(selectTrigger("Bottom button")).toHaveTextContent("Select dereferenced");
    fireEvent.click(selectTrigger("Bottom button")); expect(screen.getByRole("option", { name: "Select dereferenced" })).toBeInTheDocument(); fireEvent.keyDown(window, { key: "Escape" });
    expect(selectTrigger("Fader")).toBeDisabled();
    expect(selectTrigger("Fader")).toHaveTextContent("Group intensity master");
    fireEvent.click(screen.getByRole("button", { name: "Apply" }));
    await waitFor(() => expect(mocks.savePlaybackSlot).toHaveBeenCalledWith(2, 4, expect.objectContaining({ target: { type: "group", group_id: "group-1" }, buttons: ["select", "flash", "select_dereferenced"], fader: "master" })));
  });

  it("renders exactly one control and no fader for a virtual topology", () => {
    show({ ...base, buttons: ["toggle", "none", "none"], button_count: 1, has_fader: false }, { virtual: true });
    expect(screen.getByText("1 button · faderless")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Playback Layout" }));
    expect(selectTrigger("Top button")).toHaveTextContent("Toggle");
    expect(screen.queryByText("Middle button", { selector: "label", exact: true })).not.toBeInTheDocument();
    expect(screen.queryByText("Fader", { selector: "label", exact: true })).not.toBeInTheDocument();
    expect(screen.getByText("No fader on this playback.")).toBeInTheDocument();
  });

  it.each(["programmer_fade", "cue_fade"] as const)("makes every button inert and fixes the %s time-master fader", (type) => {
    const playback = withFunctionDefaults(base, type, "cue-1", "group-1"); show(playback);
    fireEvent.click(screen.getByRole("button", { name: "Playback Layout" }));
    for (const label of ["Top button", "Middle button", "Bottom button"]) { expect(selectTrigger(label)).toBeDisabled(); expect(selectTrigger(label)).toHaveTextContent("Disabled"); }
    expect(selectTrigger("Fader")).toBeDisabled();
    expect(selectTrigger("Fader")).toHaveTextContent(type === "programmer_fade" ? "Programmer Fade time" : "Cue Fade time");
  });

  it("persists function, layout and color atomically and Cancel performs no mutation", async () => {
    const close = show();
    fireEvent.click(screen.getByRole("button", { name: "Playback color #8b5cf6" }));
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(close).toHaveBeenCalledOnce(); expect(mocks.savePlaybackSlot).not.toHaveBeenCalled();
    cleanup(); show(); fireEvent.click(screen.getByRole("button", { name: "Playback color #8b5cf6" })); fireEvent.click(screen.getByRole("button", { name: "Apply" }));
    await waitFor(() => expect(mocks.savePlaybackSlot).toHaveBeenCalledWith(2, 4, expect.objectContaining({ color: "#8b5cf6", button_count: 3, has_fader: true })));
  });

  it("requires confirmation for Clear and cancellation leaves the playback intact", async () => {
    show(); fireEvent.click(screen.getByRole("button", { name: "Clear Playback" })); fireEvent.click(screen.getByRole("button", { name: "Keep Playback" }));
    expect(mocks.clearPlaybackSlot).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Clear Playback" })); fireEvent.click(screen.getByRole("button", { name: "Confirm Clear Playback" }));
    await waitFor(() => expect(mocks.clearPlaybackSlot).toHaveBeenCalledWith(2, 4));
  });

  it("does not offer Clear for an empty slot and sends number zero to atomic allocation", async () => {
    show({ ...base, number: 0, name: "Playback 2.4" }, { empty: true });
    expect(screen.queryByRole("button", { name: "Clear Playback" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Apply" }));
    await waitFor(() => expect(mocks.savePlaybackSlot).toHaveBeenCalledWith(2, 4, expect.objectContaining({ number: 0 })));
  });

  it("persists mutually exclusive virtual icon and image-background presentation", async () => {
    show({ ...base, button_count: 1, has_fader: false }, { virtual: true });
    choose("Presentation", "Image background");
    fireEvent.change(screen.getByLabelText("Image background"), { target: { value: "show://images/blue-wash.png" } });
    fireEvent.click(screen.getByRole("button", { name: "Apply" }));
    await waitFor(() => expect(mocks.savePlaybackSlot).toHaveBeenCalledWith(2, 4, expect.objectContaining({ presentation_image: "show://images/blue-wash.png", presentation_icon: undefined, button_count: 1, has_fader: false })));
  });

  it("migrates legacy topology deterministically and clears hidden button actions", () => {
    const migrated = normalizePlaybackTopology({ ...base, button_count: undefined, has_fader: undefined }, 2, false);
    expect(migrated).toMatchObject({ button_count: 2, has_fader: false, buttons: ["go_minus", "go", "none"] });
  });
});
