import { createPortal } from "react-dom";
import { useMemo, useState } from "react";
import type { PlaybackButtonAction, PlaybackDefinition } from "../../api/types";
import { useServer } from "../../api/ServerContext";
import { Button, ColorPickerField, FormField, FormLayout, MultiValueToggleField, SelectField, SwitchField, TextField } from "../common";
import { ModalTitleBar } from "../common/ModalTitleBar";
import { SelectionTree, WindowScrollArea, type SelectionListOption } from "../window-kit";
import { useShowObjectView } from "../../features/showObjects/ShowObjectsView";
import { useCueLists } from "../../features/showObjects/ShowObjectsState";
import { useGroups } from "../../features/server/useShowObjectsState";

export const PLAYBACK_COLORS = ["#ef4444", "#f97316", "#f59e0b", "#eab308", "#84cc16", "#22c55e", "#20c997", "#06b6d4", "#0ea5e9", "#3b82f6", "#6366f1", "#8b5cf6", "#a855f7", "#ec4899", "#f43f5e", "#f8fafc"] as const;

type PlaybackTab = "function" | "behavior" | "layout";
type PlaybackFamily = "cue_list" | "group" | "speed_group" | "special" | "none";
type LayoutChoice = PlaybackButtonAction | PlaybackDefinition["fader"];

const cueActions: PlaybackButtonAction[] = ["go_minus", "go", "fast_forward", "fast_rewind", "pause", "on", "off", "toggle", "flash", "temp", "swap", "select", "select_contents"];
const buttonLabels: Record<PlaybackButtonAction, string> = {
  on: "On", off: "Off", toggle: "Toggle", go: "GO +", go_minus: "GO −", fast_forward: "FFW", fast_rewind: "FRW",
  flash: "Flash", temp: "Temp", swap: "Swap", select: "Select", select_contents: "Select contents", select_dereferenced: "Select dereferenced",
  learn: "Learn", double: "Double", half: "Half", pause: "Pause", blackout: "Blackout", pause_dynamics: "Pause Dynamics", none: "Empty Button",
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

export function PlaybackConfigurationModal({ playback, page, slot, empty = false, virtual = false, onClose }: PlaybackConfigurationModalProps) {
  useShowObjectView("group");
  useShowObjectView("cue_list");
  const server = useServer();
  const groups = useGroups(server.playbacks);
  const cueListObjects = useCueLists();
  const cueLists = useMemo(() => cueListObjects.map(({ id, body }) => ({ id, name: body.name || id })), [cueListObjects]);
  const [initialDraft] = useState(() => normalizePlaybackTopology(playback, virtual ? 1 : server.playbacks?.desk.buttons ?? 3, !virtual));
  const [draft, setDraft] = useState(initialDraft);
  const initialFamily = familyFromTarget(initialDraft.target.type);
  const [family, setFamily] = useState<PlaybackFamily>(initialFamily);
  const [tab, setTab] = useState<PlaybackTab>("function");
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);
  const presentation = draft.presentation_image ? "image" : draft.presentation_icon ? "icon" : "label";
  const targetValid = family === "none" || ((draft.target.type !== "cue_list" || Boolean(draft.target.cue_list_id)) && (draft.target.type !== "group" || Boolean(draft.target.group_id)));
  const currentPayload = cleanPresentation(normalizePlaybackTopology(draft, draft.button_count ?? 3, Boolean(draft.has_fader)));
  const initialPayload = cleanPresentation(initialDraft);
  const isDirty = family === "none" ? !empty : family !== initialFamily || !playbackDefinitionsEqual(currentPayload, initialPayload);
  const topology = `${draft.button_count ?? 3} button${draft.button_count === 1 ? "" : "s"} · ${draft.has_fader ? "fader" : "faderless"}`;
  const options = useMemo(() => layoutActions(draft).map((value) => ({ value, label: buttonLabels[value], description: layoutActionDescription(draft, value) })), [draft.target.type]);

  const apply = async () => {
    setBusy(true); setFailure(null);
    const succeeded = family === "none"
      ? await server.clearPlaybackSlot(page, slot)
      : await server.savePlaybackSlot(page, slot, cleanPresentation(normalizePlaybackTopology(draft, draft.button_count ?? 3, Boolean(draft.has_fader))));
    setBusy(false);
    if (succeeded) onClose();
    else setFailure(server.error ?? (family === "none" ? "Playback could not be cleared." : "Playback configuration could not be saved."));
  };
  const chooseFamily = (next: PlaybackFamily) => {
    setFamily(next);
    if (next === "none") return;
    const type = next === "special" ? (isSpecial(draft.target.type) ? draft.target.type : "programmer_fade") : next;
    if (type !== draft.target.type) setDraft(withFunctionDefaults(draft, type, cueLists[0]?.id ?? "", groups[0]?.id ?? ""));
  };
  const chooseSpecial = (type: "programmer_fade" | "cue_fade" | "grand_master") => {
    if (type !== draft.target.type) setDraft(withFunctionDefaults(draft, type, cueLists[0]?.id ?? "", groups[0]?.id ?? ""));
  };

  return createPortal(<div className="stacked-modal-layer" onPointerDown={(event) => event.target === event.currentTarget && onClose()}>
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
        {tab === "function" && <PlaybackFunctionTab family={family} draft={draft} virtual={virtual} presentation={presentation} cueLists={cueLists} groups={groups} onFamilyChange={chooseFamily} onSpecialChange={chooseSpecial} onDraftChange={setDraft}/>}
        {tab === "behavior" && <WindowScrollArea className="playback-tab-scroll"><div className="playback-tab-scroll-content">{family === "none" ? <InactivePlaybackDetail/> : <PlaybackBehaviorTab draft={draft} onDraftChange={setDraft}/>}</div></WindowScrollArea>}
        {tab === "layout" && <WindowScrollArea className="playback-tab-scroll"><div className="playback-tab-scroll-content">{family === "none" ? <InactivePlaybackDetail/> : <PlaybackLayoutTab draft={draft} options={options} onDraftChange={setDraft}/>}</div></WindowScrollArea>}
        {failure && <p role="alert" className="modal-error">{failure}</p>}
      </div>
    </section>
  </div>, document.body);
}

function PlaybackFunctionTab({ family, draft, virtual, presentation, cueLists, groups, onFamilyChange, onSpecialChange, onDraftChange }: {
  family: PlaybackFamily;
  draft: PlaybackDefinition;
  virtual: boolean;
  presentation: string;
  cueLists: Array<{ id: string; name: string }>;
  groups: ReadonlyArray<{ id: string; body: { name?: string } }>;
  onFamilyChange: (family: PlaybackFamily) => void;
  onSpecialChange: (type: "programmer_fade" | "cue_fade" | "grand_master") => void;
  onDraftChange: (playback: PlaybackDefinition) => void;
}) {
  const functionOptions: SelectionListOption[] = [{ value: "cue_list", label: "Cue List" }, { value: "group", label: "Group Master" }, { value: "speed_group", label: "Speed Master" }, { value: "special", label: "Special" }, { value: "none", label: "None", tone: "danger" }];
  let optionValue: string | undefined;
  let optionLabel = `${family === "cue_list" ? "Cue List" : family === "group" ? "Group" : family === "speed_group" ? "Speed Group" : "Special"} options`;
  let targetOptions: SelectionListOption[] = [];
  let chooseTarget = (_value: string) => {};
  if (family === "cue_list" && draft.target.type === "cue_list") { optionValue = draft.target.cue_list_id; targetOptions = cueLists.map((cue) => ({ value: cue.id, label: cue.name })); chooseTarget = (cue_list_id) => onDraftChange({ ...draft, target: { type: "cue_list", cue_list_id } }); }
  else if (family === "group" && draft.target.type === "group") { optionValue = draft.target.group_id; targetOptions = groups.map((group) => ({ value: group.id, label: group.body.name ?? group.id })); chooseTarget = (group_id) => onDraftChange({ ...draft, target: { type: "group", group_id } }); }
  else if (family === "speed_group" && draft.target.type === "speed_group") { optionValue = draft.target.group; targetOptions = ["A", "B", "C", "D", "E"].map((value) => ({ value, label: `Speed Group ${value}` })); chooseTarget = (group) => onDraftChange({ ...draft, target: { type: "speed_group", group } }); }
  else if (family === "special") { optionValue = isSpecial(draft.target.type) ? draft.target.type : "programmer_fade"; targetOptions = [{ value: "programmer_fade", label: "Programmer Fade" }, { value: "cue_fade", label: "Cue Fade" }, { value: "grand_master", label: "Grand Master" }]; chooseTarget = (value) => onSpecialChange(value as "programmer_fade" | "cue_fade" | "grand_master"); }
  const presentationOptions = virtual && family !== "none" ? <FormLayout className="playback-presentation-options"><SelectField label="Presentation" value={presentation} onChange={(value) => onDraftChange({ ...draft, presentation_icon: value === "icon" ? draft.presentation_icon ?? "▶" : undefined, presentation_image: value === "image" ? draft.presentation_image ?? "image://playback" : undefined })} options={[{ value: "label", label: "Label" }, { value: "icon", label: "Icon" }, { value: "image", label: "Image background" }]}/>{presentation === "icon" && <TextField label="Icon" value={draft.presentation_icon ?? ""} maxLength={1024} onChange={(event) => onDraftChange({ ...draft, presentation_icon: event.target.value, presentation_image: undefined })}/>} {presentation === "image" && <TextField label="Image background" value={draft.presentation_image ?? ""} maxLength={1024} onChange={(event) => onDraftChange({ ...draft, presentation_image: event.target.value, presentation_icon: undefined })}/>}</FormLayout> : undefined;
  return <div className="playback-function-screen">
    <SelectionTree className={`playback-function-tree ${family === "none" ? "has-inactive-detail" : ""}`} columns={[
      { id: "function", title: "Function", ariaLabel: "Playback function", value: family, options: functionOptions, onChange: (value) => onFamilyChange(value as PlaybackFamily) },
      { id: "options", title: "Options", ariaLabel: optionLabel, value: optionValue, options: targetOptions, onChange: chooseTarget, emptyLabel: family === "none" ? "Playback will be cleared" : "No options are available", footer: presentationOptions },
    ]}/>
    <section className={`playback-function-identity ${family === "none" ? "inactive" : ""}`}>
      <FormLayout columns={2} minColumnWidth={220}>
        <TextField label="Playback name" value={draft.name} maxLength={80} disabled={family === "none"} onChange={(event) => onDraftChange({ ...draft, name: event.target.value })}/>
        <ColorPickerField label="Playback color" colors={[...PLAYBACK_COLORS]} value={draft.color ?? "#20c997"} disabled={family === "none"} onChange={(color) => onDraftChange({ ...draft, color })}/>
      </FormLayout>
    </section>
  </div>;
}

function PlaybackBehaviorTab({ draft, onDraftChange }: { draft: PlaybackDefinition; onDraftChange: (playback: PlaybackDefinition) => void }) {
  const cueList = draft.target.type === "cue_list";
  return <FormLayout labelPlacement="side">
    {cueList ? <>
      <MultiValueToggleField label="When Flash or Swap is released" description="Release all removes the temporary values and restores the prior state. Intensity only leaves this Cue List active at zero intensity, retaining values such as color and position." value={draft.flash_release ?? "release_all"} onChange={(flash_release) => onDraftChange({ ...draft, flash_release })} options={[{ value: "release_all", label: "Release all" }, { value: "release_intensity_only", label: "Intensity only" }]}/>
      <SwitchField label="Turn off when other playbacks take full control" description="Automatically turns this Cue List off once other normal playbacks at full level control every value it was outputting. Partial takeovers, Flash, and Temp do not count." checked={draft.auto_off} onChange={(event) => onDraftChange({ ...draft, auto_off: event.target.checked })}/>
    </> : <p className="playback-topology-note">Flash/Swap release and automatic turn-off are available for Cue Lists only.</p>}
    <SwitchField label="Protect from Swap" description="Keeps this playback at its current level while another playback’s Swap button is held." checked={Boolean(draft.protect_from_swap)} onChange={(event) => onDraftChange({ ...draft, protect_from_swap: event.target.checked })}/>
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
  const [open, setOpen] = useState(false);
  const selected = options.find((option) => option.value === value);
  const groups = groupLayoutChoices(kind, options);
  const emptyButton = kind === "button" && value === "none";
  return <FormField label={label}><Button className={`ui-select-trigger playback-layout-choice-trigger ${emptyButton ? "is-empty" : ""}`} disabled={disabled} aria-haspopup="dialog" aria-expanded={open} onClick={() => setOpen(true)}><span>{emptyButton ? "Empty Button" : selected?.label ?? value}</span><i aria-hidden="true">›</i></Button>{open && createPortal(<div className="stacked-modal-layer playback-layout-choice-layer" onPointerDown={(event) => event.target === event.currentTarget && setOpen(false)}><section className="nested-modal playback-layout-choice-modal" role="dialog" aria-modal="true" aria-label={`Choose ${label} function`}><ModalTitleBar title={`Choose ${label} function`} actions={kind === "button" ? <Button variant="danger" onClick={() => { onChange("none"); setOpen(false); }}>Empty Button</Button> : undefined} closeLabel={`Close ${label} function choices`} onClose={() => setOpen(false)}/><WindowScrollArea className="playback-layout-choice-scroll"><div className="playback-layout-choice-groups">{groups.map((group) => <section key={group.label}><h3>{group.label}</h3><div className="playback-layout-choice-options">{group.options.map((option) => <Button key={option.value} active={option.value === value} onClick={() => { onChange(option.value); setOpen(false); }}><b>{option.label}</b><small>{option.description}</small></Button>)}</div></section>)}</div></WindowScrollArea></section></div>, document.body)}</FormField>;
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

export function withFunctionDefaults(playback: PlaybackDefinition, type: string, cueListId: string, groupId: string): PlaybackDefinition {
  let target: PlaybackDefinition["target"];
  let buttons: PlaybackDefinition["buttons"];
  let fader: PlaybackDefinition["fader"];
  if (type === "cue_list") { target = { type, cue_list_id: cueListId }; buttons = ["go_minus", "go", "flash"]; fader = "master"; }
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
function fixedFaderLabel(playback: PlaybackDefinition) { if (playback.target.type === "group") return "Group intensity master"; if (playback.target.type === "grand_master") return "Grand Master"; if (playback.target.type === "programmer_fade") return "Programmer Fade time"; if (playback.target.type === "cue_fade") return "Cue Fade time"; return null; }
function fixedFaderDescription(playback: PlaybackDefinition) { if (playback.target.type === "group") return "Controls the assigned Group's intensity master."; if (playback.target.type === "grand_master") return "Controls the global Grand Master."; if (playback.target.type === "programmer_fade") return "Controls the Programmer Fade time master."; if (playback.target.type === "cue_fade") return "Controls the Cue Fade time master."; return null; }
function cleanPresentation(playback: PlaybackDefinition): PlaybackDefinition { const presentation_icon = playback.presentation_icon?.trim() || undefined; const presentation_image = playback.presentation_image?.trim() || undefined; return { ...playback, presentation_icon, presentation_image: presentation_icon ? undefined : presentation_image }; }
function playbackDefinitionsEqual(left: PlaybackDefinition, right: PlaybackDefinition) { return JSON.stringify(left) === JSON.stringify(right); }
