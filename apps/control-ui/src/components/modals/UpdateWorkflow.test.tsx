import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { UpdateMenuEntry, UpdateMode, UpdatePreview, UpdateSettings, UpdateTargetRequest } from "../../api/types";
import { UpdateOperationDialog, UpdateSettingsDialog, UpdateTargetMenu, updatePreviewStats } from "./UpdateWorkflow";
import { configuredUpdateMode, cueUpdateTarget, defaultUpdateSettings } from "../control/updateWorkflow";

const request: UpdateTargetRequest = cueUpdateTarget("cue-list-a", 7, { id: "cue-2", number: 2 });
const existingOnly: UpdateMode = { target_type: "cue", mode: "existing_only" };
const target = { family: { type: "cue" as const }, object_id: "cue-list-a", name: "Main Cuelist", playback_number: 7, cue: { id: "cue-2", number: 2 } };
const preview: UpdatePreview = {
  revision: 4,
  programmer_revision: "programmer-a",
  target,
  mode: existingOnly,
  items: [
    { address: { type: "fixture_attribute", fixture_id: "fixture-1", attribute: "intensity" }, outcome: { outcome: "change_at_source", source: { cue_id: "cue-1", cue_number: 1, cue_index: 0 } } },
    { address: { type: "fixture_attribute", fixture_id: "fixture-1", attribute: "color.red" }, outcome: { outcome: "ignored", reason: "new_address" } },
  ],
};

describe("Update workflow", () => {
  it("shows the four literal Cue modes and authoritative source/ignored preview before applying", () => {
    const onMode = vi.fn();
    const onApply = vi.fn();
    const onCancel = vi.fn();
    render(<UpdateOperationDialog operation={{ request, preview }} busy={false} error={null} onMode={onMode} onApply={onApply} onCancel={onCancel}/>);

    const dialog = screen.getByRole("dialog", { name: "Update Main Cuelist" });
    expect(within(dialog).getByText("Cuelist · Playback 7 · Current Cue 2")).toBeInTheDocument();
    for (const label of ["Existing Only", "Existing in Current Cue", "Add to Current Cue", "Add New"]) expect(within(dialog).getByRole("button", { name: label })).toBeInTheDocument();
    expect(within(dialog).getByText("Change at source Cue 1")).toBeInTheDocument();
    expect(within(dialog).getByText("Ignored · address is new to this target")).toBeInTheDocument();
    expect(updatePreviewStats(preview)).toMatchObject({ eligible: 1, changed: 1, ignored: 1, source: 1 });

    fireEvent.click(within(dialog).getByRole("button", { name: "Add to Current Cue" }));
    expect(onMode).toHaveBeenCalledWith({ target_type: "cue", mode: "add_to_current_cue" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Update Cuelist" }));
    expect(onApply).toHaveBeenCalledTimes(1);
    fireEvent.click(within(dialog).getByRole("button", { name: "Cancel" }));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("keeps deterministic desk defaults separate from show programming", () => {
    const onChange = vi.fn();
    const onSave = vi.fn();
    render(<UpdateSettingsDialog settings={defaultUpdateSettings} busy={false} error={null} onChange={onChange} onSave={onSave} onCancel={vi.fn()}/>);
    const dialog = screen.getByRole("dialog", { name: "Update Settings" });
    expect(within(dialog).getByText("Desk workflow preferences for Update. These settings do not change show programming.")).toBeInTheDocument();
    expect(within(dialog).getByRole("button", { name: /Add to Current Cue/ })).toBeInTheDocument();
    expect(within(dialog).getAllByRole("button", { name: /Update Existing/ })).toHaveLength(2);
    fireEvent.click(within(dialog).getByRole("switch", { name: "Show Update modal on touch" }));
    expect(onChange).toHaveBeenCalledWith(expect.objectContaining({ show_update_modal_on_touch: false }));
    fireEvent.click(within(dialog).getByRole("button", { name: "Save Update Settings" }));
    expect(onSave).toHaveBeenCalledTimes(1);
  });

  it("distinguishes eligible targets from visible no-ops and applies the shown concrete context", () => {
    const noOp: UpdatePreview = { ...preview, target: { ...target, object_id: "cue-list-b", name: "No-op Cuelist", playback_number: 8 }, items: [{ address: { type: "fixture_attribute", fixture_id: "fixture-1", attribute: "intensity" }, outcome: { outcome: "unchanged" } }] };
    const entries: UpdateMenuEntry[] = [
      { revision: 4, target, active_or_referenced: true, existing_preview: preview, add_new_preview: { ...preview, mode: { target_type: "cue", mode: "add_new" } } },
      { revision: 8, target: noOp.target, active_or_referenced: true, existing_preview: noOp, add_new_preview: { ...noOp, mode: { target_type: "cue", mode: "add_new" } } },
    ];
    const onFilter = vi.fn();
    const onApply = vi.fn();
    render(<UpdateTargetMenu entries={entries} filter="eligible_for_update_existing" modes={{}} busyKey={null} error={null} onFilter={onFilter} onMode={vi.fn()} onApply={onApply} onCancel={vi.fn()}/>);
    const dialog = screen.getByRole("dialog", { name: "Update Update" });
    expect(within(dialog).getByText("Cuelist · Playback 7 · Current Cue 2")).toBeInTheDocument();
    expect(within(dialog).getByText("No-op Cuelist").closest("article")).toHaveTextContent("No eligible change");
    expect(within(dialog).getByRole("button", { name: "No changes" })).toBeDisabled();
    fireEvent.click(within(dialog).getByRole("button", { name: "Show All Active" }));
    expect(onFilter).toHaveBeenCalledWith("show_all_active");
    fireEvent.click(within(dialog).getAllByRole("button", { name: "Update" })[0]);
    expect(onApply).toHaveBeenCalledWith(entries[0], existingOnly);
  });

  it("maps each target family to its configured default without changing the target", () => {
    const settings: UpdateSettings = { ...defaultUpdateSettings, cue_mode: "existing_in_current_cue", preset_mode: "add_new", group_mode: "update_existing" };
    expect(configuredUpdateMode(settings, request)).toEqual({ target_type: "cue", mode: "existing_in_current_cue" });
    expect(configuredUpdateMode(settings, { family: { type: "preset" }, object_id: "4" })).toEqual({ target_type: "existing_content", mode: "add_new" });
    expect(configuredUpdateMode(settings, { family: { type: "group" }, object_id: "3" })).toEqual({ target_type: "existing_content", mode: "update_existing" });
  });
});
