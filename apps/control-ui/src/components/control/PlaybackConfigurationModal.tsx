import { createPortal } from "react-dom";
import { useMemo, useState } from "react";
import type { PlaybackButtonAction, PlaybackDefinition } from "../../api/types";
import { useServer } from "../../api/ServerContext";
import { Button, FormLayout, SelectField, SwitchField, TextField } from "../common";
import { ModalTitleBar } from "../common/ModalTitleBar";

export const PLAYBACK_COLORS = ["#ef4444", "#f97316", "#f59e0b", "#eab308", "#84cc16", "#22c55e", "#20c997", "#06b6d4", "#0ea5e9", "#3b82f6", "#6366f1", "#8b5cf6", "#a855f7", "#ec4899", "#f43f5e", "#f8fafc"] as const;

const cueActions: PlaybackButtonAction[] = ["go_minus", "go", "fast_forward", "fast_rewind", "pause", "on", "off", "toggle", "flash", "temp", "swap", "select", "select_contents", "none"];
const buttonLabels: Record<PlaybackButtonAction, string> = {
  on: "On", off: "Off", toggle: "Toggle", go: "Go plus", go_minus: "Go minus", fast_forward: "Fast forward", fast_rewind: "Fast rewind",
  flash: "Flash", temp: "Temp", swap: "Swap", select: "Select", select_contents: "Select contents", select_dereferenced: "Select dereferenced",
  learn: "Learn", double: "Double", half: "Half", pause: "Pause", blackout: "Blackout", pause_dynamics: "Pause Dynamics", none: "Disabled",
};
const faderLabels: Record<PlaybackDefinition["fader"], string> = {
  master: "Master", temp: "Temp", speed: "Speed", x_fade: "X-fade", direct_bpm: "Direct BPM", centered_relative: "Centered relative", learned_percentage: "Learned-speed percentage",
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
  const server = useServer();
  const [draft, setDraft] = useState(() => normalizePlaybackTopology(playback, virtual ? 1 : server.playbacks?.desk.buttons ?? 3, !virtual));
  const [tab, setTab] = useState<"function" | "layout">("function");
  const [confirmClear, setConfirmClear] = useState(false);
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);
  const presentation = draft.presentation_image ? "image" : draft.presentation_icon ? "icon" : "label";
  const targetValid = (draft.target.type !== "cue_list" || Boolean(draft.target.cue_list_id)) && (draft.target.type !== "group" || Boolean(draft.target.group_id));
  const topology = `${draft.button_count ?? 3} button${draft.button_count === 1 ? "" : "s"} · ${draft.has_fader ? "fader" : "faderless"}`;
  const options = useMemo(() => layoutActions(draft).map((value) => ({ value, label: buttonLabels[value] })), [draft.target.type]);

  const save = async () => {
    setBusy(true); setFailure(null);
    const payload = cleanPresentation(normalizePlaybackTopology(draft, draft.button_count ?? 3, Boolean(draft.has_fader)));
    const saved = await server.savePlaybackSlot(page, slot, payload);
    setBusy(false);
    if (saved) onClose(); else setFailure(server.error ?? "Playback configuration could not be saved.");
  };
  const clear = async () => {
    setBusy(true); setFailure(null);
    const cleared = await server.clearPlaybackSlot(page, slot);
    setBusy(false);
    if (cleared) onClose(); else setFailure(server.error ?? "Playback could not be cleared.");
  };

  return createPortal(<div className="stacked-modal-layer" onPointerDown={(event) => event.target === event.currentTarget && onClose()}>
    <section className="nested-modal playback-configuration-modal" role="dialog" aria-modal="true" aria-label="Playback Configuration" data-page={page} data-slot={slot} data-topology={topology}>
      <ModalTitleBar title={`Playback Configuration · ${page}.${slot}`} onClose={onClose} closeLabel="Cancel playback configuration" />
      <div className="playback-configuration-identity"><b>Page {page} · Playback {slot}</b><span>{topology}</span>{empty && <small>Empty slot — Apply creates the assignment atomically.</small>}</div>
      <nav className="segmented-control"><Button className={tab === "function" ? "active" : ""} onClick={() => setTab("function")}>Playback Function</Button><Button className={tab === "layout" ? "active" : ""} onClick={() => setTab("layout")}>Playback Layout</Button></nav>
      <div className="playback-configuration-body">
        {tab === "function" ? <FormLayout labelPlacement="side">
          <TextField label="Playback name" value={draft.name} maxLength={80} onChange={(event) => setDraft({ ...draft, name: event.target.value })}/>
          <SelectField label="Function" value={draft.target.type} onChange={(type) => setDraft(withFunctionDefaults(draft, type, server.playbacks?.cue_lists[0]?.id ?? "", server.groups[0]?.id ?? ""))} options={[{ value: "cue_list", label: "Cuelist" }, { value: "group", label: "Group Master" }, { value: "speed_group", label: "Speed Master / Speed Group" }, { value: "programmer_fade", label: "Programmer Fade" }, { value: "cue_fade", label: "Cue Fade" }, { value: "grand_master", label: "Grand Master" }]}/>
          {draft.target.type === "cue_list" && <SelectField label="Cuelist" value={draft.target.cue_list_id} onChange={(cue_list_id) => setDraft({ ...draft, target: { type: "cue_list", cue_list_id } })} options={server.playbacks?.cue_lists.map((cue) => ({ value: cue.id, label: cue.name })) ?? []}/>}
          {draft.target.type === "group" && <SelectField label="Group" value={draft.target.group_id} onChange={(group_id) => setDraft({ ...draft, target: { type: "group", group_id } })} options={server.groups.map((group) => ({ value: group.id, label: group.body.name ?? group.id }))}/>}
          {draft.target.type === "speed_group" && <SelectField label="Speed Group" value={draft.target.group} onChange={(group) => setDraft({ ...draft, target: { type: "speed_group", group } })} options={["A", "B", "C", "D", "E"].map((value) => ({ value, label: `Speed Group ${value}` }))}/>}
          {draft.target.type === "cue_list" && <><SelectField label="Flash release" value={draft.flash_release ?? "release_all"} onChange={(flash_release) => setDraft({ ...draft, flash_release: flash_release as PlaybackDefinition["flash_release"] })} options={[{ value: "release_all", label: "Release all" }, { value: "release_intensity_only", label: "Release intensity only" }]}/><SwitchField label="Switch Cuelist off when fully overwritten" checked={draft.auto_off} onChange={(event) => setDraft({ ...draft, auto_off: event.target.checked })}/><SwitchField label="Protect from Swap" checked={Boolean(draft.protect_from_swap)} onChange={(event) => setDraft({ ...draft, protect_from_swap: event.target.checked })}/></>}
          {virtual && <><SelectField label="Presentation" value={presentation} onChange={(value) => setDraft({ ...draft, presentation_icon: value === "icon" ? draft.presentation_icon ?? "▶" : undefined, presentation_image: value === "image" ? draft.presentation_image ?? "image://playback" : undefined })} options={[{ value: "label", label: "Label" }, { value: "icon", label: "Icon" }, { value: "image", label: "Image background" }]}/>{presentation === "icon" && <TextField label="Icon" value={draft.presentation_icon ?? ""} maxLength={1024} onChange={(event) => setDraft({ ...draft, presentation_icon: event.target.value, presentation_image: undefined })}/>} {presentation === "image" && <TextField label="Image background" value={draft.presentation_image ?? ""} maxLength={1024} onChange={(event) => setDraft({ ...draft, presentation_image: event.target.value, presentation_icon: undefined })}/>}</>}
        </FormLayout> : <FormLayout labelPlacement="side">
          {Array.from({ length: draft.button_count ?? 3 }, (_, index) => <SelectField key={index} label={["Top button", "Middle button", "Bottom button"][index]} value={draft.buttons[index]} disabled={isTimeMaster(draft)} onChange={(value) => { const next = [...draft.buttons] as PlaybackDefinition["buttons"]; next[index] = value as PlaybackButtonAction; setDraft({ ...draft, buttons: next }); }} options={isTimeMaster(draft) ? [{ value: "none", label: "Disabled" }] : options}/>) }
          {draft.button_count === 0 && <p className="playback-topology-note">This playback has no buttons.</p>}
          {draft.has_fader ? <SelectField label="Fader" value={draft.fader} disabled={fixedFader(draft)} onChange={(fader) => setDraft({ ...draft, fader: fader as PlaybackDefinition["fader"] })} options={faderModes(draft).map((value) => ({ value, label: fixedFaderLabel(draft) ?? faderLabels[value] }))}/> : <p className="playback-topology-note">No fader on this playback.</p>}
          {draft.target.type === "speed_group" && draft.fader === "centered_relative" && <p className="playback-topology-note">50% is exactly 1× the learned speed; lower travel slows and higher travel speeds up.</p>}
        </FormLayout>}
        <h3>Playback color</h3><div className="playback-color-palette">{PLAYBACK_COLORS.map((color) => <Button key={color} aria-label={`Playback color ${color}`} className={(draft.color ?? "#20c997").toLowerCase() === color ? "active" : ""} style={{ backgroundColor: color }} onClick={() => setDraft({ ...draft, color })}/>)}</div>
        {failure && <p role="alert" className="modal-error">{failure}</p>}
      </div>
      <footer className="modal-actions"><Button disabled={busy} onClick={onClose}>Cancel</Button>{!empty && (confirmClear ? <><Button disabled={busy} onClick={() => setConfirmClear(false)}>Keep Playback</Button><Button variant="danger" disabled={busy} onClick={() => void clear()}>{busy ? "Clearing…" : "Confirm Clear Playback"}</Button></> : <Button variant="danger" disabled={busy} onClick={() => setConfirmClear(true)}>Clear Playback</Button>)}<Button variant="primary" disabled={busy || !targetValid || !draft.name.trim()} onClick={() => void save()}>{busy ? "Applying…" : "Apply"}</Button></footer>
    </section>
  </div>, document.body);
}

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
  else if (type === "group") { target = { type, group_id: groupId }; buttons = ["select", "flash", "select_dereferenced"]; fader = "master"; }
  else if (type === "speed_group") { target = { type, group: "A" }; buttons = ["double", "half", "learn"]; fader = "learned_percentage"; }
  else if (type === "programmer_fade") { target = { type }; buttons = ["none", "none", "none"]; fader = "master"; }
  else if (type === "cue_fade") { target = { type }; buttons = ["none", "none", "none"]; fader = "master"; }
  else { target = { type: "grand_master" }; buttons = ["blackout", "flash", "pause_dynamics"]; fader = "master"; }
  return normalizePlaybackTopology({ ...playback, target, buttons, fader }, playback.button_count ?? 3, Boolean(playback.has_fader));
}

function layoutActions(playback: PlaybackDefinition): PlaybackButtonAction[] {
  if (playback.target.type === "cue_list") return cueActions;
  if (playback.target.type === "speed_group") return ["double", "half", "learn", "pause", "none"];
  if (playback.target.type === "group") return ["select", "flash", "select_dereferenced", "none"];
  if (playback.target.type === "grand_master") return ["blackout", "flash", "pause_dynamics", "none"];
  return ["none"];
}
function faderModes(playback: PlaybackDefinition): PlaybackDefinition["fader"][] { if (playback.target.type === "cue_list") return ["master", "x_fade", "temp"]; if (playback.target.type === "speed_group") return ["learned_percentage", "direct_bpm", "centered_relative"]; return ["master"]; }
function isTimeMaster(playback: PlaybackDefinition) { return playback.target.type === "programmer_fade" || playback.target.type === "cue_fade"; }
function fixedFader(playback: PlaybackDefinition) { return !["cue_list", "speed_group"].includes(playback.target.type); }
function fixedFaderLabel(playback: PlaybackDefinition) { if (playback.target.type === "group") return "Group intensity master"; if (playback.target.type === "grand_master") return "Grand Master"; if (playback.target.type === "programmer_fade") return "Programmer Fade time"; if (playback.target.type === "cue_fade") return "Cue Fade time"; return null; }
function cleanPresentation(playback: PlaybackDefinition): PlaybackDefinition { const presentation_icon = playback.presentation_icon?.trim() || undefined; const presentation_image = playback.presentation_image?.trim() || undefined; return { ...playback, presentation_icon, presentation_image: presentation_icon ? undefined : presentation_image }; }
