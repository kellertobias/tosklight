import { createPortal } from "react-dom";
import { useEffect, useMemo, useState } from "react";
import type { PlaybackButtonAction, PlaybackDefinition } from "../../api/types";
import { Button, ColorPickerField, DEFAULT_COLORS, FormLayout, GroupedSelectionField, ModalRegistration, MultiValueToggleField, NumberField, SelectField, SwitchField, TextField } from "@tosklight/ui";
import { ModalTitleBar } from "@tosklight/ui";
import { SelectionTree, WindowScrollArea, type SelectionListOption } from "@tosklight/ui/window-kit";
import { useShowObjectView } from "../../features/showObjects/ShowObjectsView";
import { useCueLists, useDynamics, usePortableGroups } from "../../features/showObjects/ShowObjectsState";
import { RootConfinedFilePickerButton } from "../files/RootConfinedFilePickerButton";

export const PLAYBACK_COLORS = DEFAULT_COLORS;

type PlaybackTab = "function" | "behavior" | "layout";
type PlaybackFamily = "cue_list" | "dynamic" | "group" | "speed_group" | "special" | "none";
type LayoutChoice = PlaybackButtonAction | PlaybackDefinition["fader"];

const cueActions: PlaybackButtonAction[] = ["go_minus", "go", "fast_forward", "fast_rewind", "pause", "on", "off", "toggle", "flash", "temp", "swap", "select", "select_contents"];
const buttonLabels: Record<PlaybackButtonAction, string> = {
  on: "On", off: "Off", toggle: "Toggle", go: "GO +", go_minus: "GO −", fast_forward: "FFW", fast_rewind: "FRW",
  flash: "Flash", temp: "Temp", swap: "Swap", select: "Select", select_contents: "Select contents", select_dereferenced: "Select dereferenced",
  learn: "Learn", double: "Double", half: "Half", pause: "Pause", blackout: "Blackout", pause_dynamics: "Pause Dynamics", none: "Empty Button",
  dynamic_restart: "Restart", dynamic_double_speed: "Double Dynamic Speed", dynamic_half_speed: "Half Dynamic Speed", dynamic_learn_speed: "Learn Dynamic Speed",
};
const buttonDescriptions: Record<PlaybackButtonAction, string> = {
  on: "Activates the playback at full virtual level without moving the physical fader.",
  off: "Releases the playback while retaining its physical fader position.",
  toggle: "Alternates the playback between its normal On and Off behavior.",
  go: "Advances to the next cue using its configured timing.",
  go_minus: "Returns to the previous cue using its configured timing.",
  fast_forward: "Advances to the next cue with fade and delay bypassed for this transition.",
  fast_rewind: "Returns to the previous cue with fade and delay bypassed for this transition.",
  flash: "Applies the playback temporarily while the button is held.",
  temp: "Toggles a temporary, non-destructive playback contribution.",
  swap: "Flashes this playback while temporarily forcing unprotected playbacks to zero.",
  select: "Selects this playback or its live Group reference without executing it.",
  select_contents: "Selects the fixtures and live Group references used by the cue list.",
  select_dereferenced: "Selects the Group's current members as individual fixtures.",
  learn: "Learns the selected Speed Group rate from repeated taps.",
  double: "Doubles the selected Speed Group rate.",
  half: "Halves the selected Speed Group rate.",
  pause: "Pauses speed or phase advancement without discarding the learned rate.",
  blackout: "Toggles global blackout.",
  pause_dynamics: "Pauses or resumes Effects and Dynamics without deleting their setup.",
  dynamic_restart: "Restarts this Dynamic Playback using its configured activation policy.",
  dynamic_double_speed: "Doubles this Dynamic Playback's local speed multiplier.",
  dynamic_half_speed: "Halves this Dynamic Playback's local speed multiplier.",
  dynamic_learn_speed: "Learns this Dynamic Playback's cycle duration from repeated taps.",
  none: "Leaves this physical button without an assigned action.",
};
const faderLabels: Record<PlaybackDefinition["fader"], string> = {
  master: "Master", temp: "Temp", speed: "Speed", x_fade: "X-fade", direct_bpm: "Direct BPM", centered_relative: "Centered relative", learned_percentage: "Learned-speed percentage",
};
const faderDescriptions: Record<PlaybackDefinition["fader"], string> = {
  master: "Controls the assigned playback or master level.",
  temp: "Applies a temporary playback contribution continuously with fader travel.",
  speed: "Controls playback speed.",
  x_fade: "Manually progresses between the current cue and the next cue.",
  direct_bpm: "Maps the fader directly from 0 to 300 BPM.",
  centered_relative: "Uses the center as 1× learned speed, slower below and faster above.",
  learned_percentage: "Maps the fader from Pause through half speed to the learned speed.",
};

export interface PlaybackConfigurationModalProps {
  playback: PlaybackDefinition;
  page: number;
  slot: number;
  empty?: boolean;
  virtual?: boolean;
  onClose: () => void;
}

export interface PlaybackConfigurationDialogProps extends PlaybackConfigurationModalProps {
  fallbackButtons: number;
  save: (page: number, slot: number, playback: PlaybackDefinition) => Promise<boolean>;
  clear: (page: number, slot: number) => Promise<boolean>;
  error?: string | null;
}

/** Shared modal body; callers choose the physical or scoped topology mutation boundary. */
export function PlaybackConfigurationDialog({ playback, page, slot, empty = false, virtual = false, fallbackButtons, save, clear, error, onClose }: PlaybackConfigurationDialogProps) {
  useShowObjectView("group");
  useShowObjectView("cue_list");
  useShowObjectView("dynamic");
  const groups = usePortableGroups();
  const dynamics = useDynamics();
  const cueListObjects = useCueLists();
  const cueLists = useMemo(() => cueListObjects.map(({ body }) => ({ id: body.id, name: body.name || body.id })), [cueListObjects]);
  const [initialDraft] = useState(() => normalizePlaybackTopology(playback, fallbackButtons, !virtual));
  const [draft, setDraft] = useState(initialDraft);
  const initialFamily = familyFromTarget(initialDraft.target.type);
  const [family, setFamily] = useState<PlaybackFamily>(initialFamily);
  const [tab, setTab] = useState<PlaybackTab>("function");
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);
  useEffect(() => {
    if (failure && error && failure !== error) setFailure(error);
  }, [error, failure]);
  const [presentation, setPresentation] = useState<"label" | "icon" | "image">(
    () => draft.presentation_image ? "image" : draft.presentation_icon ? "icon" : "label",
  );
  const selectedDynamicId = draft.target.type === "dynamic" ? draft.target.assignment.dynamic_id : undefined;
  const selectedDynamic = selectedDynamicId ? dynamics.find((dynamic) => dynamic.id === selectedDynamicId) : undefined;
  const dynamicScopeValid = draft.target.type !== "dynamic" || selectedDynamic?.body.target_binding.type !== "targetless"
    || (draft.target.assignment.target_scope?.type === "live_group" ? Boolean(draft.target.assignment.target_scope.group_id) : (draft.target.assignment.target_scope?.targets.length ?? 0) > 0);
  const targetValid = family === "none" || ((draft.target.type !== "cue_list" || Boolean(draft.target.cue_list_id)) && (draft.target.type !== "group" || Boolean(draft.target.group_id)) && (draft.target.type !== "dynamic" || Boolean(draft.target.assignment.dynamic_id)) && dynamicScopeValid);
  const currentPayload = cleanPresentation(normalizePlaybackTopology(draft, draft.button_count ?? 3, Boolean(draft.has_fader)));
  const initialPayload = cleanPresentation(initialDraft);
  const isDirty = family === "none" ? !empty : family !== initialFamily || !playbackDefinitionsEqual(currentPayload, initialPayload);
  const topology = `${draft.button_count ?? 3} button${draft.button_count === 1 ? "" : "s"} · ${draft.has_fader ? "fader" : "faderless"}`;
  const options = useMemo(() => layoutActions(draft).map((value) => ({ value, label: buttonLabels[value], description: layoutActionDescription(draft, value) })), [draft.target.type]);

  const apply = async () => {
    setBusy(true); setFailure(null);
    const succeeded = family === "none"
      ? await clear(page, slot)
      : await save(page, slot, cleanPresentation(normalizePlaybackTopology(draft, draft.button_count ?? 3, Boolean(draft.has_fader))));
    setBusy(false);
    if (succeeded) onClose();
    else setFailure(error ?? (family === "none" ? "Playback could not be cleared." : "Playback configuration could not be saved."));
  };
  const chooseFamily = (next: PlaybackFamily) => {
    setFamily(next);
    if (next === "none") return;
    const type = next === "special" ? (isSpecial(draft.target.type) ? draft.target.type : "programmer_fade") : next;
    if (type !== draft.target.type) setDraft(withFunctionDefaults(draft, type, cueLists[0]?.id ?? "", groups[0]?.id ?? "", dynamics[0]));
  };
  const chooseSpecial = (type: "programmer_fade" | "cue_fade" | "grand_master") => {
    if (type !== draft.target.type) setDraft(withFunctionDefaults(draft, type, cueLists[0]?.id ?? "", groups[0]?.id ?? "", dynamics[0]));
  };

  return createPortal(<ModalRegistration onClose={onClose}><div className="stacked-modal-layer" onPointerDown={(event) => event.target === event.currentTarget && onClose()}>
    <section className="nested-modal playback-configuration-modal" role="dialog" aria-modal="true" aria-label="Playback Configuration" data-page={page} data-slot={slot} data-topology={topology}>
      <ModalTitleBar
        title={`Playback Configuration · ${page}.${slot}`}
        actions={<Button variant="primary" disabled={busy || !isDirty || !targetValid || (family !== "none" && !draft.name.trim())} onClick={() => void apply()}>{busy ? (family === "none" ? "Clearing…" : "Applying…") : "Apply"}</Button>}
        onClose={onClose}
        closeLabel="Close playback configuration"
      />
      <nav className="segmented-control playback-configuration-tabs">
        <Button className={tab === "function" ? "active" : ""} onClick={() => setTab("function")}>Function</Button>
        <Button className={tab === "behavior" ? "active" : ""} onClick={() => setTab("behavior")}>Behavior</Button>
        <Button className={tab === "layout" ? "active" : ""} onClick={() => setTab("layout")}>Layout</Button>
      </nav>
      <div className="playback-configuration-body">
        {tab === "function" && <PlaybackFunctionTab family={family} draft={draft} virtual={virtual} presentation={presentation} cueLists={cueLists} dynamics={dynamics} groups={groups} onFamilyChange={chooseFamily} onSpecialChange={chooseSpecial} onPresentationChange={setPresentation} onDraftChange={setDraft}/>}
        {tab === "behavior" && <WindowScrollArea className="playback-tab-scroll"><div className="playback-tab-scroll-content">{family === "none" ? <InactivePlaybackDetail/> : <PlaybackBehaviorTab draft={draft} dynamics={dynamics} groups={groups} onDraftChange={setDraft}/>}</div></WindowScrollArea>}
        {tab === "layout" && <WindowScrollArea className="playback-tab-scroll"><div className="playback-tab-scroll-content">{family === "none" ? <InactivePlaybackDetail/> : <PlaybackLayoutTab draft={draft} options={options} onDraftChange={setDraft}/>}</div></WindowScrollArea>}
        {failure && <p role="alert" className="modal-error">{failure}</p>}
      </div>
    </section>
  </div></ModalRegistration>, document.body);
}

function PlaybackFunctionTab({ family, draft, virtual, presentation, cueLists, dynamics, groups, onFamilyChange, onSpecialChange, onPresentationChange, onDraftChange }: {
  family: PlaybackFamily;
  draft: PlaybackDefinition;
  virtual: boolean;
  presentation: "label" | "icon" | "image";
  cueLists: Array<{ id: string; name: string }>;
  dynamics: ReturnType<typeof useDynamics>;
  groups: ReadonlyArray<{ id: string; body: { name?: string } }>;
  onFamilyChange: (family: PlaybackFamily) => void;
  onSpecialChange: (type: "programmer_fade" | "cue_fade" | "grand_master") => void;
  onPresentationChange: (presentation: "label" | "icon" | "image") => void;
  onDraftChange: (playback: PlaybackDefinition) => void;
}) {
  const functionOptions: SelectionListOption[] = [{ value: "cue_list", label: "Cue List" }, ...(dynamics.length > 0 ? [{ value: "dynamic", label: "Dynamic" }] : []), { value: "group", label: "Group Master" }, { value: "speed_group", label: "Speed Master" }, { value: "special", label: "Special" }, { value: "none", label: "None", tone: "danger" }];
  let optionValue: string | undefined;
  let optionLabel = `${family === "cue_list" ? "Cue List" : family === "group" ? "Group" : family === "speed_group" ? "Speed Group" : "Special"} options`;
  let targetOptions: SelectionListOption[] = [];
  let chooseTarget = (_value: string) => {};
  if (family === "cue_list" && draft.target.type === "cue_list") { optionValue = draft.target.cue_list_id; targetOptions = cueLists.map((cue) => ({ value: cue.id, label: cue.name })); chooseTarget = (cue_list_id) => onDraftChange({ ...draft, target: { type: "cue_list", cue_list_id } }); }
  else if (family === "dynamic" && draft.target.type === "dynamic") { const assignment = draft.target.assignment; optionLabel = "Dynamic options"; optionValue = assignment.dynamic_id ?? undefined; targetOptions = dynamics.map((dynamic) => ({ value: dynamic.id, label: `Dynamic ${dynamic.body.pool_number} · ${dynamic.body.name}${dynamic.body.target_binding.type === "targetless" ? " · targetless" : ""}` })); chooseTarget = (dynamicId) => { const dynamic = dynamics.find((candidate) => candidate.id === dynamicId); if (!dynamic) return; onDraftChange({ ...draft, target: { type: "dynamic", assignment: { ...assignment, dynamic_id: dynamic.id, last_known_pool_number: dynamic.body.pool_number, embedded_fallback: dynamic.body, target_scope: dynamic.body.target_binding.type === "targetless" ? assignment.target_scope : null, revision: assignment.revision + 1 } } }); }; }
  else if (family === "group" && draft.target.type === "group") { const groupTarget = draft.target; optionValue = groupTarget.group_id; targetOptions = groups.map((group) => ({ value: group.id, label: group.body.name ?? group.id })); chooseTarget = (group_id) => onDraftChange({ ...draft, target: { type: "group", group_id, ...(group_id === groupTarget.group_id && groupTarget.initial_master != null ? { initial_master: groupTarget.initial_master } : {}) } }); }
  else if (family === "speed_group" && draft.target.type === "speed_group") { optionValue = draft.target.group; targetOptions = ["A", "B", "C", "D", "E"].map((value) => ({ value, label: `Speed Group ${value}` })); chooseTarget = (group) => onDraftChange({ ...draft, target: { type: "speed_group", group } }); }
  else if (family === "special") { optionValue = isSpecial(draft.target.type) ? draft.target.type : "programmer_fade"; targetOptions = [{ value: "programmer_fade", label: "Programmer Fade" }, { value: "cue_fade", label: "Cue Fade" }, { value: "grand_master", label: "Grand Master" }]; chooseTarget = (value) => onSpecialChange(value as "programmer_fade" | "cue_fade" | "grand_master"); }
  const presentationOptions = virtual && family !== "none" ? <FormLayout className="playback-presentation-options"><SelectField label="Presentation" value={presentation} onChange={(value) => {
    onPresentationChange(value);
    onDraftChange({ ...draft, presentation_icon: undefined, presentation_image: undefined });
  }} options={[{ value: "label", label: "Label" }, { value: "icon", label: "Icon" }, { value: "image", label: "Image background" }]}/>{presentation === "icon" && <TextField label="Icon" value={draft.presentation_icon ?? ""} maxLength={1024} onChange={(event) => onDraftChange({ ...draft, presentation_icon: event.target.value, presentation_image: undefined })}/>} {presentation === "image" && <div className="playback-image-setting">{draft.presentation_image ? <img src={draft.presentation_image} alt="Selected playback background"/> : <small>No image selected.</small>}<RootConfinedFilePickerButton label={draft.presentation_image ? "Change image" : "Choose image"} allowedExtensions={["png", "jpg", "jpeg", "gif", "webp"]} onFiles={async (files) => {
    const file = files[0];
    if (!file) return;
    const presentation_image = await playbackImageDataUrl(file);
    onDraftChange({ ...draft, presentation_image, presentation_icon: undefined });
  }}/>{draft.presentation_image && <Button onClick={() => onDraftChange({ ...draft, presentation_image: undefined })}>Remove image</Button>}</div>}</FormLayout> : undefined;
  return <div className="playback-function-screen">
    <SelectionTree className={`playback-function-tree ${family === "none" ? "has-inactive-detail" : ""}`} columns={[
      { id: "function", title: "Function", ariaLabel: "Playback function", value: family, options: functionOptions, onChange: (value) => onFamilyChange(value as PlaybackFamily) },
      { id: "options", title: "Options", ariaLabel: optionLabel, value: optionValue, options: targetOptions, onChange: chooseTarget, emptyLabel: family === "none" ? "Playback will be cleared" : "No options are available", footer: presentationOptions },
    ]}/>
    <section className={`playback-function-identity ${family === "none" ? "inactive" : ""}`}>
      <FormLayout columns={2} minColumnWidth={220}>
        <TextField label="Playback name" value={draft.name} maxLength={80} disabled={family === "none"} onChange={(event) => onDraftChange({ ...draft, name: event.target.value })}/>
        <ColorPickerField label="Playback color" value={draft.color ?? "#20c997"} disabled={family === "none"} onChange={(color) => onDraftChange({ ...draft, color })}/>
      </FormLayout>
    </section>
  </div>;
}

const MAX_PLAYBACK_IMAGE_BYTES = 400 * 1024;

export function playbackImageDataUrl(file: File): Promise<string> {
  if (!file.type.startsWith("image/"))
    return Promise.reject(new Error("Choose a PNG, JPEG, GIF, or WebP image."));
  if (file.size > MAX_PLAYBACK_IMAGE_BYTES)
    return Promise.reject(new Error("Playback images must be 400 KB or smaller."));
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onerror = () => reject(new Error("The selected image could not be read."));
    reader.onload = () => {
      if (typeof reader.result === "string") resolve(reader.result);
      else reject(new Error("The selected image could not be read."));
    };
    reader.readAsDataURL(file);
  });
}

function PlaybackBehaviorTab({ draft, dynamics, groups, onDraftChange }: { draft: PlaybackDefinition; dynamics: ReturnType<typeof useDynamics>; groups: ReadonlyArray<{ id: string; body: { name?: string } }>; onDraftChange: (playback: PlaybackDefinition) => void }) {
  const cueList = draft.target.type === "cue_list";
  if (draft.target.type === "dynamic") {
    const assignment = draft.target.assignment;
    const update = (patch: Partial<typeof assignment>) => onDraftChange({ ...draft, target: { type: "dynamic", assignment: { ...assignment, ...patch } } });
    const definition = dynamics.find((dynamic) => dynamic.id === assignment.dynamic_id)?.body ?? assignment.embedded_fallback;
    const targetless = definition.target_binding.type === "targetless";
    const targetScope = assignment.target_scope;
    const frozenTargets = targetScope?.type === "frozen_targets" ? targetScope.targets.join(", ") : "";
    return <FormLayout labelPlacement="side">
      <div className="playback-topology-note">
        <b>{assignment.dynamic_id ? `Dynamic ${assignment.last_known_pool_number} · ${definition.name}` : `Embedded Dynamic ${assignment.last_known_pool_number}`}</b>
        <span>{definition.target_binding.type === "live_group" ? `Live Group ${definition.target_binding.group_id}` : definition.target_binding.type === "frozen_targets" ? `${definition.target_binding.targets.length} frozen target${definition.target_binding.targets.length === 1 ? "" : "s"}` : targetScope?.type === "live_group" ? `Targetless · Live Group ${targetScope.group_id}` : targetScope?.type === "frozen_targets" ? `Targetless · ${targetScope.targets.length} frozen target${targetScope.targets.length === 1 ? "" : "s"}` : "Targetless · assignment scope required"}</span>
      </div>
      {targetless && <>
        <SelectField label="Target scope" value={targetScope?.type ?? "unassigned"} onChange={(value) => update({ target_scope: value === "live_group" ? { type: "live_group", group_id: "" } : value === "frozen_targets" ? { type: "frozen_targets", targets: [] } : null })} options={[{ value: "unassigned", label: "Choose a target scope" }, { value: "live_group", label: "Live Group" }, { value: "frozen_targets", label: "Frozen ordered targets" }]}/>
        {targetScope?.type === "live_group" && <SelectField label="Live Group" value={targetScope.group_id} onChange={(group_id) => update({ target_scope: { type: "live_group", group_id } })} options={groups.map((group) => ({ value: group.id, label: group.body.name ? `${group.id} · ${group.body.name}` : group.id }))}/>}
        {targetScope?.type === "frozen_targets" && <TextField label="Frozen target UUIDs" value={frozenTargets} onChange={(event) => update({ target_scope: { type: "frozen_targets", targets: event.target.value.split(",").map((target) => target.trim()).filter(Boolean) } })} description="Ordered fixture/head UUIDs, separated by commas. The assignment will not fall back to all fixtures."/>}
        {!targetScope && <p role="alert" className="modal-error">Choose a reviewable Live Group or frozen ordered target scope before applying this targetless Dynamic.</p>}
      </>}
      <SelectField label="Fader control" value={assignment.fader_mode} onChange={(fader_mode) => update({ fader_mode })} options={[{ value: "none", label: "None" }, { value: "master", label: "Master" }, { value: "size", label: "Size" }, { value: "size_and_master", label: "Size + Master" }]}/>
      <NumberField label="Dynamic priority" value={assignment.priority} min={-32768} max={32767} step={1} onChange={(event) => update({ priority: Number(event.target.value) })}/>
      <SelectField label="Activation policy" value={assignment.activation_override ?? "follow_dynamic"} onChange={(value) => update({ activation_override: value === "follow_dynamic" ? null : value as typeof assignment.activation_override })} options={[{ value: "follow_dynamic", label: "Follow Dynamic" }, { value: "start_now", label: "Start now" }, { value: "join_sync_now", label: "Join sync now" }, { value: "next_boundary", label: "Next boundary" }]}/>
      <SelectField label="Resume policy" value={assignment.resume_policy} onChange={(resume_policy) => update({ resume_policy })} options={[{ value: "follow_dynamic", label: "Follow Dynamic" }, { value: "resume_frozen_phase", label: "Resume frozen phase" }, { value: "rejoin_synchronized_position", label: "Rejoin synchronized position" }, { value: "resume_on_next_boundary", label: "Resume on next boundary" }]}/>
      <NumberField label="Local speed numerator" value={assignment.local_speed_multiplier.numerator} min={1} max={1024} step={1} onChange={(event) => update({ local_speed_multiplier: { ...assignment.local_speed_multiplier, numerator: Number(event.target.value) } })}/>
      <NumberField label="Local speed denominator" value={assignment.local_speed_multiplier.denominator} min={1} max={1024} step={1} onChange={(event) => update({ local_speed_multiplier: { ...assignment.local_speed_multiplier, denominator: Number(event.target.value) } })}/>
      <SwitchField label="Crossfade non-intensity" offLabel="Switch by LTP" onLabel="Crossfade" description="When Master contributes to this fader mode, crossfade Position, Color, Beam, and Focus instead of switching them at full ownership." checked={assignment.crossfade_non_intensity} onChange={(event) => update({ crossfade_non_intensity: event.target.checked })}/>
      <SwitchField label="Auto-off at fader zero" offLabel="Keep synchronized" onLabel="Turn Off" checked={assignment.auto_off_at_zero} onChange={(event) => update({ auto_off_at_zero: event.target.checked })}/>
      <SwitchField label="Auto-off after Flash release" offLabel="Return to prior state" onLabel="Turn Off" checked={assignment.auto_off_flash_release} onChange={(event) => update({ auto_off_flash_release: event.target.checked })}/>
      <SwitchField label="Turn off when other playbacks take full control" offLabel="Keep hidden" onLabel="Auto off" checked={assignment.auto_off_full_control} onChange={(event) => update({ auto_off_full_control: event.target.checked })}/>
      <SwitchField label="Protect from Swap" offLabel="Affected by Swap" onLabel="Protected" checked={Boolean(draft.protect_from_swap)} onChange={(event) => onDraftChange({ ...draft, protect_from_swap: event.target.checked })}/>
    </FormLayout>;
  }
  return <FormLayout labelPlacement="side">
    {cueList ? <>
      <MultiValueToggleField label="When Flash or Swap is released" description="Release all removes the temporary values and restores the prior state. Intensity only leaves this Cue List active at zero intensity, retaining values such as color and position." value={draft.flash_release ?? "release_all"} onChange={(flash_release) => onDraftChange({ ...draft, flash_release })} options={[{ value: "release_all", label: "Release all" }, { value: "release_intensity_only", label: "Intensity only" }]}/>
      <SwitchField label="Turn off when other playbacks take full control" offLabel="Keep active" onLabel="Auto off" description="Automatically turns this Cue List off once other normal playbacks at full level control every value it was outputting. Partial takeovers, Flash, and Temp do not count." checked={draft.auto_off} onChange={(event) => onDraftChange({ ...draft, auto_off: event.target.checked })}/>
    </> : <p className="playback-topology-note">Flash/Swap release and automatic turn-off are available for Cue Lists only.</p>}
    <SwitchField label="Protect from Swap" offLabel="Affected by Swap" onLabel="Protected" description="Keeps this playback at its current level while another playback’s Swap button is held." checked={Boolean(draft.protect_from_swap)} onChange={(event) => onDraftChange({ ...draft, protect_from_swap: event.target.checked })}/>
  </FormLayout>;
}

function PlaybackLayoutTab({ draft, options, onDraftChange }: { draft: PlaybackDefinition; options: Array<{ value: PlaybackButtonAction; label: string; description: string }>; onDraftChange: (playback: PlaybackDefinition) => void }) {
  return <FormLayout labelPlacement="side">
    {Array.from({ length: draft.button_count ?? 3 }, (_, index) => <LayoutChoiceField kind="button" key={index} label={["Top button", "Middle button", "Bottom button"][index]} value={draft.buttons[index]} options={options} onChange={(value) => { const next = [...draft.buttons] as PlaybackDefinition["buttons"]; next[index] = value as PlaybackButtonAction; onDraftChange({ ...draft, buttons: next }); }}/>) }
    {draft.button_count === 0 && <p className="playback-topology-note">This playback has no buttons.</p>}
    {draft.has_fader ? <LayoutChoiceField kind="fader" label="Fader" value={draft.fader} disabled={fixedFader(draft)} onChange={(fader) => onDraftChange({ ...draft, fader: fader as PlaybackDefinition["fader"] })} options={faderModes(draft).map((value) => ({ value, label: fixedFaderLabel(draft) ?? faderLabels[value], description: fixedFaderDescription(draft) ?? faderDescriptions[value] }))}/> : <p className="playback-topology-note">No fader on this playback.</p>}
    {draft.target.type === "speed_group" && draft.fader === "centered_relative" && <p className="playback-topology-note">50% is exactly 1× the learned speed; lower travel slows and higher travel speeds up.</p>}
  </FormLayout>;
}

function LayoutChoiceField({ kind, label, value, options, onChange, disabled = false }: { kind: "button" | "fader"; label: string; value: LayoutChoice; options: Array<{ value: LayoutChoice; label: string; description: string }>; onChange: (value: LayoutChoice) => void; disabled?: boolean }) {
  const groups = groupLayoutChoices(kind, options);
  return <GroupedSelectionField
    label={label}
    dialogTitle={`Choose ${label} function`}
    closeLabel={`Close ${label} function choices`}
    value={value}
    groups={groups}
    onChange={onChange}
    disabled={disabled}
    clearAction={kind === "button" ? { label: "Empty Button", value: "none" } : undefined}
  />;
}

function groupLayoutChoices(kind: "button" | "fader", options: Array<{ value: LayoutChoice; label: string; description: string }>) {
  const groups: Array<{ label: string; options: typeof options }> = [];
  const add = (label: string, option: typeof options[number]) => { const group = groups.find((candidate) => candidate.label === label); if (group) group.options.push(option); else groups.push({ label, options: [option] }); };
  const timeButtons = kind === "button" && options.some((option) => option.value === "off") && options.some((option) => option.value === "double" || option.value === "half");
  const speedButtons = kind === "button" && !timeButtons && options.some((option) => option.value === "double" || option.value === "half" || option.value === "learn");
  const grandMasterButtons = kind === "button" && options.some((option) => option.value === "blackout" || option.value === "pause_dynamics");
  for (const option of options) {
    if (timeButtons) add("Time Control", option);
    else if (speedButtons) add("Speed Control", option);
    else if (grandMasterButtons) add("Grand Master Control", option);
    else if (kind === "fader") add(option.value === "x_fade" ? "Cue Transition" : ["direct_bpm", "centered_relative", "learned_percentage", "speed"].includes(option.value) ? "Speed Control" : "Level Control", option);
    else if (["go", "go_minus", "fast_forward", "fast_rewind", "pause"].includes(option.value)) add("Step Control", option);
    else if (["on", "off", "toggle"].includes(option.value)) add("Permanent State", option);
    else if (["flash", "temp", "swap"].includes(option.value)) add("Temporary State", option);
    else add("Selection", option);
  }
  return groups;
}

function InactivePlaybackDetail() { return <div className="playback-cleared-message"><b>Playback will be cleared</b><span>Apply to remove this playback assignment. Closing the modal keeps it unchanged.</span></div>; }

export function normalizePlaybackTopology(playback: PlaybackDefinition, fallbackButtons: number, fallbackFader: boolean): PlaybackDefinition {
  const buttonCount = Math.max(0, Math.min(3, playback.button_count ?? fallbackButtons)) as 0 | 1 | 2 | 3;
  const buttons = playback.buttons.map((action, index) => index < buttonCount ? action : "none") as PlaybackDefinition["buttons"];
  return { ...playback, buttons, button_count: buttonCount, has_fader: playback.has_fader ?? fallbackFader, color: playback.color ?? "#20c997", flash_release: playback.flash_release ?? "release_all", protect_from_swap: Boolean(playback.protect_from_swap) };
}

export function withFunctionDefaults(playback: PlaybackDefinition, type: string, cueListId: string, groupId: string, dynamic?: ReturnType<typeof useDynamics>[number]): PlaybackDefinition {
  let target: PlaybackDefinition["target"];
  let buttons: PlaybackDefinition["buttons"];
  let fader: PlaybackDefinition["fader"];
  if (type === "cue_list") { target = { type, cue_list_id: cueListId }; buttons = ["go_minus", "go", "flash"]; fader = "master"; }
  else if (type === "dynamic" && dynamic) { target = { type, assignment: { dynamic_id: dynamic.id, last_known_pool_number: dynamic.body.pool_number, embedded_fallback: dynamic.body, revision: 1, target_scope: dynamic.body.target_binding.type === "targetless" ? null : undefined, fader_mode: "size_and_master", priority: 0, activation_override: null, resume_policy: "follow_dynamic", local_speed_multiplier: { numerator: 1, denominator: 1 }, learned_duration_millis: null, crossfade_non_intensity: false, auto_off_at_zero: false, auto_off_flash_release: false, auto_off_full_control: true } }; buttons = ["off", "pause", "flash"]; fader = "master"; }
  else if (type === "group") { target = { type, group_id: groupId }; buttons = ["select", "select_dereferenced", "flash"]; fader = "master"; }
  else if (type === "speed_group") { target = { type, group: "A" }; buttons = ["double", "half", "learn"]; fader = "learned_percentage"; }
  else if (type === "programmer_fade") { target = { type }; buttons = ["double", "half", "off"]; fader = "master"; }
  else if (type === "cue_fade") { target = { type }; buttons = ["double", "half", "off"]; fader = "master"; }
  else { target = { type: "grand_master" }; buttons = ["blackout", "pause_dynamics", "flash"]; fader = "master"; }
  return normalizePlaybackTopology({ ...playback, target, buttons, fader }, playback.button_count ?? 3, Boolean(playback.has_fader));
}

function familyFromTarget(type: PlaybackDefinition["target"]["type"]): PlaybackFamily { return isSpecial(type) ? "special" : type; }
function isSpecial(type: PlaybackDefinition["target"]["type"]): type is "programmer_fade" | "cue_fade" | "grand_master" { return type === "programmer_fade" || type === "cue_fade" || type === "grand_master"; }
function layoutActions(playback: PlaybackDefinition): PlaybackButtonAction[] {
  if (playback.target.type === "cue_list") return cueActions;
  if (playback.target.type === "dynamic") return ["on", "off", "toggle", "dynamic_restart", "pause", "dynamic_double_speed", "dynamic_half_speed", "dynamic_learn_speed", "flash", "temp", "swap"];
  if (playback.target.type === "speed_group") return ["double", "half", "learn", "pause"];
  if (playback.target.type === "group") return ["select", "select_dereferenced", "flash"];
  if (playback.target.type === "programmer_fade" || playback.target.type === "cue_fade") return ["double", "half", "off"];
  if (playback.target.type === "grand_master") return ["blackout", "pause_dynamics", "flash"];
  return [];
}
function faderModes(playback: PlaybackDefinition): PlaybackDefinition["fader"][] { if (playback.target.type === "cue_list") return ["master", "x_fade", "temp"]; if (playback.target.type === "speed_group") return ["learned_percentage", "direct_bpm", "centered_relative"]; return ["master"]; }
function layoutActionDescription(playback: PlaybackDefinition, action: PlaybackButtonAction) {
  if (playback.target.type === "programmer_fade" || playback.target.type === "cue_fade") {
    if (action === "double") return "Doubles the current fade time.";
    if (action === "half") return "Halves the current fade time.";
    if (action === "off") return "Sets the fade time to zero, disabling the fade delay.";
  }
  return buttonDescriptions[action];
}
function fixedFader(playback: PlaybackDefinition) { return !["cue_list", "speed_group"].includes(playback.target.type); }
function fixedFaderLabel(playback: PlaybackDefinition) { if (playback.target.type === "dynamic") return `Dynamic ${playback.target.assignment.fader_mode.replaceAll("_", " + ")}`; if (playback.target.type === "group") return "Group intensity master"; if (playback.target.type === "grand_master") return "Grand Master"; if (playback.target.type === "programmer_fade") return "Programmer Fade time"; if (playback.target.type === "cue_fade") return "Cue Fade time"; return null; }
function fixedFaderDescription(playback: PlaybackDefinition) { if (playback.target.type === "dynamic") return "The Dynamic assignment maps this physical fader through its configured Size/Master mode."; if (playback.target.type === "group") return "Controls the assigned Group's intensity master."; if (playback.target.type === "grand_master") return "Controls the global Grand Master."; if (playback.target.type === "programmer_fade") return "Controls the Programmer Fade time master."; if (playback.target.type === "cue_fade") return "Controls the Cue Fade time master."; return null; }
function cleanPresentation(playback: PlaybackDefinition): PlaybackDefinition { const presentation_icon = playback.presentation_icon?.trim() || undefined; const presentation_image = playback.presentation_image?.trim() || undefined; return { ...playback, presentation_icon, presentation_image: presentation_icon ? undefined : presentation_image }; }
function playbackDefinitionsEqual(left: PlaybackDefinition, right: PlaybackDefinition) { return JSON.stringify(left) === JSON.stringify(right); }
