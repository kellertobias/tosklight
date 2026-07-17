import type {
  CueUpdateMode,
  ExistingContentMode,
  UpdateMode,
  UpdateSettings,
  UpdateTargetIdentity,
  UpdateTargetRequest,
} from "../../api/types";

export const UPDATE_TARGET_EVENT = "light:update-target";
export const UPDATE_ARMED_EVENT = "light:update-armed";
export const UPDATE_SETTINGS_EVENT = "light:update-settings";
export const UPDATE_TARGET_MENU_EVENT = "light:update-target-menu";

export const cueUpdateModes: Array<{ value: CueUpdateMode; label: string }> = [
  { value: "existing_only", label: "Existing Only" },
  { value: "existing_in_current_cue", label: "Existing in Current Cue" },
  { value: "add_to_current_cue", label: "Add to Current Cue" },
  { value: "add_new", label: "Add New" },
];

export const existingContentModes: Array<{ value: ExistingContentMode; label: string }> = [
  { value: "update_existing", label: "Update Existing" },
  { value: "add_new", label: "Add New" },
];

export const defaultUpdateSettings: UpdateSettings = {
  cue_mode: "add_to_current_cue",
  preset_mode: "update_existing",
  group_mode: "update_existing",
  other_target_modes: {},
  show_update_modal_on_touch: true,
};

export function configuredUpdateMode(settings: UpdateSettings, target: UpdateTargetRequest): UpdateMode {
  if (target.family.type === "cue") return { target_type: "cue", mode: settings.cue_mode };
  const mode = target.family.type === "preset"
    ? settings.preset_mode
    : target.family.type === "group"
      ? settings.group_mode
      : settings.other_target_modes[target.family.kind] ?? "update_existing";
  return { target_type: "existing_content", mode };
}

export function cueUpdateTarget(
  objectId: string,
  playbackNumber?: number,
  cue?: { id: string; number: number } | null,
): UpdateTargetRequest {
  return {
    family: { type: "cue" },
    object_id: objectId,
    ...(playbackNumber == null ? {} : { playback_number: playbackNumber }),
    ...(cue ? { cue_id: cue.id, cue_number: cue.number } : {}),
    ...(playbackNumber != null ? { validate_active_context: true } : {}),
  };
}

export function requestUpdateTarget(target: UpdateTargetRequest) {
  window.dispatchEvent(new CustomEvent<UpdateTargetRequest>(UPDATE_TARGET_EVENT, { detail: target }));
}

export function openUpdateSettings() {
  window.dispatchEvent(new Event(UPDATE_SETTINGS_EVENT));
}

export function openUpdateTargetMenu() {
  window.dispatchEvent(new Event(UPDATE_TARGET_MENU_EVENT));
}

export function updateTargetKey(target: UpdateTargetIdentity) {
  return [target.family.type, target.object_id, target.playback_number ?? "", target.cue?.id ?? ""].join(":");
}

export function targetFamilyLabel(target: Pick<UpdateTargetIdentity, "family">) {
  if (target.family.type === "cue") return "Cuelist";
  if (target.family.type === "preset") return "Preset";
  if (target.family.type === "group") return "Group";
  return target.family.kind;
}

export function modeLabel(mode: UpdateMode) {
  const options = mode.target_type === "cue" ? cueUpdateModes : existingContentModes;
  return options.find((candidate) => candidate.value === mode.mode)?.label ?? mode.mode.replaceAll("_", " ");
}
