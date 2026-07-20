import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { UpdateMenuEntry, UpdateResult, UpdateSettings, UpdateTargetRequest } from "../../api/types";
import {
  UPDATE_TARGET_EVENT,
  UPDATE_TARGET_MENU_EVENT,
  defaultUpdateSettings,
} from "../control/updateWorkflow";
import { UpdateWorkflow } from "./UpdateWorkflow";

const workflow = vi.hoisted(() => {
  const state = { updateArmed: false, shiftArmed: false };
  const dispatch = vi.fn((action: { type: string; value?: boolean }) => {
    if (action.type === "SET_UPDATE_ARMED") state.updateArmed = Boolean(action.value);
    if (action.type === "SET_SHIFT_ARMED") state.shiftArmed = Boolean(action.value);
  });
  return {
    state,
    dispatch,
    server: {
      error: null as string | null,
      commandLine: "",
      updateSettings: vi.fn(),
      saveUpdateSettings: vi.fn(),
      updateTargets: vi.fn(),
      previewUpdate: vi.fn(),
      applyUpdate: vi.fn(),
      setCommandLine: vi.fn(),
      resetCommandLine: vi.fn(),
    },
  };
});

vi.mock("../../api/ServerContext", () => ({ useServer: () => workflow.server }));
vi.mock("../../state/AppContext", () => ({ useApp: () => ({ state: workflow.state, dispatch: workflow.dispatch }) }));

const cueTarget = {
  family: { type: "cue" as const },
  object_id: "cue-list-a",
  name: "Main Cuelist",
  playback_number: 7,
  cue: { id: "cue-2", number: 2 },
};
const cueEntry: UpdateMenuEntry = {
  revision: 4,
  target: cueTarget,
  active_or_referenced: true,
  existing_preview: {
    revision: 4,
    show_revision: 12,
    programmer_revision: "programmer-existing",
    target: cueTarget,
    mode: { target_type: "cue", mode: "existing_only" },
    items: [{
      address: { type: "fixture_attribute", fixture_id: "fixture-1", attribute: "intensity" },
      outcome: { outcome: "change_at_source", source: { cue_id: "cue-1", cue_number: 1, cue_index: 0 } },
    }],
  },
  add_new_preview: {
    revision: 4,
    show_revision: 12,
    programmer_revision: "programmer-add-new",
    target: cueTarget,
    mode: { target_type: "cue", mode: "add_new" },
    items: [{
      address: { type: "fixture_attribute", fixture_id: "fixture-2", attribute: "color.red" },
      outcome: { outcome: "add_new_to_current_cue", cue: { cue_id: "cue-2", cue_number: 2, cue_index: 1 } },
    }],
  },
};

function resultFor(target: UpdateResult["target"] = cueTarget): UpdateResult {
  return {
    target,
    revision_before: 4,
    revision_after: 5,
    eligible_count: 1,
    changed_count: 1,
    added_count: 1,
    ignored_count: 0,
    changed_cues: [],
    programmer_values_retained: true,
  };
}

beforeEach(() => {
  workflow.state.updateArmed = false;
  workflow.state.shiftArmed = false;
  workflow.server.error = null;
  workflow.server.commandLine = "";
  vi.clearAllMocks();
  workflow.server.updateSettings.mockResolvedValue(defaultUpdateSettings);
  workflow.server.saveUpdateSettings.mockResolvedValue(defaultUpdateSettings);
  workflow.server.updateTargets.mockResolvedValue([]);
  workflow.server.previewUpdate.mockResolvedValue(null);
  workflow.server.applyUpdate.mockResolvedValue(null);
});

afterEach(cleanup);

describe("Update workflow integration", () => {
  it("rerenders Show All Active, selects Add New per target, and applies that concrete mode", async () => {
    workflow.server.updateTargets.mockResolvedValue([cueEntry]);
    workflow.server.applyUpdate.mockResolvedValue(resultFor());
    render(<UpdateWorkflow/>);

    fireEvent(window, new Event(UPDATE_TARGET_MENU_EVENT));
    const dialog = await screen.findByRole("dialog", { name: "Update Update" });
    expect(workflow.server.updateTargets).toHaveBeenCalledWith("eligible_for_update_existing");
    expect(within(dialog).queryByText("Mode for Main Cuelist", { selector: "label" })).not.toBeInTheDocument();

    fireEvent.click(within(dialog).getByRole("button", { name: "Show All Active" }));
    await waitFor(() => expect(workflow.server.updateTargets).toHaveBeenLastCalledWith("show_all_active"));
    const modeLabel = within(dialog).getByText("Mode for Main Cuelist", { selector: "label" });
    const modeTrigger = modeLabel.closest(".ui-form-field")!.querySelector(".ui-select-trigger") as HTMLButtonElement;
    expect(modeTrigger).toHaveTextContent("Existing Only");

    fireEvent.click(modeTrigger);
    fireEvent.click(screen.getByRole("option", { name: "Add New" }));
    expect(modeTrigger).toHaveTextContent("Add New");
    fireEvent.click(within(dialog).getByRole("button", { name: "Update" }));

    await waitFor(() => expect(workflow.server.applyUpdate).toHaveBeenCalledWith(
      {
        family: { type: "cue" },
        object_id: "cue-list-a",
        playback_number: 7,
        cue_id: "cue-2",
        cue_number: 2,
        validate_active_context: true,
      },
      { target_type: "cue", mode: "add_new" },
      4,
      "programmer-add-new",
      12,
    ));
    expect(await screen.findByRole("dialog", { name: "Update complete" })).toBeInTheDocument();
  });

  it("routes a touched target directly to the configured default when its modal is disabled", async () => {
    const request: UpdateTargetRequest = { family: { type: "group" }, object_id: "3" };
    const settings: UpdateSettings = {
      ...defaultUpdateSettings,
      group_mode: "add_new",
      show_update_modal_on_touch: false,
    };
    const target = { family: { type: "group" as const }, object_id: "3", name: "Group 3" };
    workflow.state.updateArmed = true;
    workflow.state.shiftArmed = true;
    workflow.server.commandLine = "UPDATE GROUP 3";
    workflow.server.updateSettings.mockResolvedValue(settings);
    workflow.server.applyUpdate.mockResolvedValue(resultFor(target));
    render(<UpdateWorkflow/>);

    expect(screen.getByRole("status")).toHaveTextContent("UPDATE armed");
    fireEvent(window, new CustomEvent<UpdateTargetRequest>(UPDATE_TARGET_EVENT, { detail: request }));

    await waitFor(() => expect(workflow.server.applyUpdate).toHaveBeenCalledWith(
      request,
      { target_type: "existing_content", mode: "add_new" },
    ));
    expect(workflow.server.previewUpdate).not.toHaveBeenCalled();
    expect(await screen.findByRole("dialog", { name: "Update complete" })).toBeInTheDocument();
    expect(workflow.dispatch).toHaveBeenCalledWith({ type: "SET_UPDATE_ARMED", value: false });
    expect(workflow.dispatch).toHaveBeenCalledWith({ type: "SET_SHIFT_ARMED", value: false });
    expect(workflow.server.resetCommandLine).toHaveBeenCalledOnce();
    expect(screen.queryByText(/UPDATE armed/)).not.toBeInTheDocument();
  });

  it("pins a cue-less touched request to the exact Cue returned by preview", async () => {
    const request: UpdateTargetRequest = {
      family: { type: "cue" },
      object_id: "cue-list-a",
      playback_number: 7,
      validate_active_context: true,
    };
    workflow.state.updateArmed = true;
    workflow.server.previewUpdate.mockResolvedValue(cueEntry.existing_preview);
    workflow.server.applyUpdate.mockResolvedValue(resultFor());
    render(<UpdateWorkflow/>);

    fireEvent(window, new CustomEvent<UpdateTargetRequest>(UPDATE_TARGET_EVENT, { detail: request }));
    const dialog = await screen.findByRole("dialog", { name: "Update Main Cuelist" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Update Cuelist" }));

    await waitFor(() => expect(workflow.server.applyUpdate).toHaveBeenCalledWith(
      {
        family: { type: "cue" },
        object_id: "cue-list-a",
        playback_number: 7,
        cue_id: "cue-2",
        cue_number: 2,
        validate_active_context: true,
      },
      cueEntry.existing_preview.mode,
      cueEntry.existing_preview.revision,
      cueEntry.existing_preview.programmer_revision,
      cueEntry.existing_preview.show_revision,
    ));
  });
});
