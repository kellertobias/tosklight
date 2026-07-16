import { createPortal } from "react-dom";
import { useState } from "react";
import type { PlaybackButtonAction, PlaybackDefinition } from "../../api/types";
import { useServer } from "../../api/ServerContext";
import { Button, FormLayout, SelectField, SwitchField } from "../common";
import { ModalTitleBar } from "../common/ModalTitleBar";

const colors = ["#ef4444", "#f97316", "#f59e0b", "#eab308", "#84cc16", "#22c55e", "#20c997", "#06b6d4", "#0ea5e9", "#3b82f6", "#6366f1", "#8b5cf6", "#a855f7", "#ec4899", "#f43f5e", "#f8fafc"];
const cueActions: PlaybackButtonAction[] = ["go_minus", "go", "fast_forward", "fast_rewind", "on", "off", "toggle", "flash", "temp", "swap", "select", "select_contents", "none"];

export function PlaybackConfigurationModal({ playback, page, slot, onUnassign, onClose }: {
  playback: PlaybackDefinition;
  page: number;
  slot: number;
  onUnassign: () => Promise<boolean>;
  onClose: () => void;
}) {
  const server = useServer();
  const [draft, setDraft] = useState(playback);
  const [tab, setTab] = useState<"function" | "layout">("function");
  const [confirmClear, setConfirmClear] = useState(false);
  const save = async () => { await server.savePlaybackDefinition(draft); onClose(); };
  return createPortal(<div className="stacked-modal-layer" onPointerDown={(event) => event.target === event.currentTarget && onClose()}>
    <section className="nested-modal playback-configuration-modal" role="dialog" aria-modal="true" aria-label="Playback Configuration">
      <ModalTitleBar title={`Playback Configuration · ${page}.${slot}`} onClose={onClose} closeLabel="Cancel playback configuration" />
      <nav className="segmented-control"><Button className={tab === "function" ? "active" : ""} onClick={() => setTab("function")}>Playback Function</Button><Button className={tab === "layout" ? "active" : ""} onClick={() => setTab("layout")}>Playback Layout</Button></nav>
      <div className="playback-configuration-body">
        {tab === "function" ? <FormLayout labelPlacement="side">
          <SelectField label="Function" value={draft.target.type} onChange={(type) => setDraft(withFunctionDefaults(draft, type, server))} options={[{ value: "cue_list", label: "Cuelist" }, { value: "group", label: "Group Master" }, { value: "speed_group", label: "Speed Master / Speed Group" }, { value: "programmer_fade", label: "Programmer Fade" }, { value: "cue_fade", label: "Cue Fade" }, { value: "grand_master", label: "Grand Master" }]}/>
          {draft.target.type === "cue_list" && <SelectField label="Cuelist" value={draft.target.cue_list_id} onChange={(cue_list_id) => setDraft({ ...draft, target: { type: "cue_list", cue_list_id } })} options={server.playbacks?.cue_lists.map((cue) => ({ value: cue.id, label: cue.name })) ?? []}/>}
          {draft.target.type === "group" && <SelectField label="Group" value={draft.target.group_id} onChange={(group_id) => setDraft({ ...draft, target: { type: "group", group_id } })} options={server.groups.map((group) => ({ value: group.id, label: group.body.name ?? group.id }))}/>}
          {draft.target.type === "speed_group" && <SelectField label="Speed Group" value={draft.target.group} onChange={(group) => setDraft({ ...draft, target: { type: "speed_group", group } })} options={["A","B","C","D","E"].map((value) => ({ value, label: `Speed Group ${value}` }))}/>}
          {draft.target.type === "cue_list" && <><SelectField label="Flash release" value={draft.flash_release ?? "release_all"} onChange={(flash_release) => setDraft({ ...draft, flash_release: flash_release as PlaybackDefinition["flash_release"] })} options={[{ value: "release_all", label: "Release all" }, { value: "release_intensity_only", label: "Release intensity only" }]}/><SwitchField label="Switch Cuelist off when fully overwritten" checked={draft.auto_off} onChange={(event) => setDraft({ ...draft, auto_off: event.target.checked })}/><SwitchField label="Protect from Swap" checked={Boolean(draft.protect_from_swap)} onChange={(event) => setDraft({ ...draft, protect_from_swap: event.target.checked })}/></>}
        </FormLayout> : <FormLayout labelPlacement="side">
          {draft.buttons.map((action, index) => <SelectField key={index} label={["Top button", "Middle button", "Bottom button"][index]} value={action} onChange={(value) => { const buttons = [...draft.buttons] as PlaybackDefinition["buttons"]; buttons[index] = value as PlaybackButtonAction; setDraft({ ...draft, buttons }); }} options={layoutActions(draft).map((value) => ({ value, label: value.replaceAll("_", " ").toUpperCase() }))}/>)}
          <SelectField label="Fader" value={draft.fader} onChange={(fader) => setDraft({ ...draft, fader: fader as PlaybackDefinition["fader"] })} options={faderModes(draft).map((value) => ({ value, label: value.replaceAll("_", " ") }))}/>
        </FormLayout>}
        <h3>Playback color</h3><div className="playback-color-palette">{colors.map((color) => <Button key={color} aria-label={`Playback color ${color}`} className={draft.color === color ? "active" : ""} style={{ backgroundColor: color }} onClick={() => setDraft({ ...draft, color })}/>)}</div>
      </div>
      <footer className="modal-actions"><Button onClick={onClose}>Cancel</Button>{confirmClear ? <><Button onClick={() => setConfirmClear(false)}>Keep Playback</Button><Button variant="danger" onClick={() => void onUnassign().then((ok) => ok && onClose())}>Confirm Clear Playback</Button></> : <Button variant="danger" onClick={() => setConfirmClear(true)}>Clear Playback</Button>}<Button variant="primary" onClick={() => void save()}>Apply</Button></footer>
    </section>
  </div>, document.body);
}

function withFunctionDefaults(playback: PlaybackDefinition, type: string, server: ReturnType<typeof useServer>): PlaybackDefinition {
  if (type === "cue_list") return { ...playback, target: { type, cue_list_id: server.playbacks?.cue_lists[0]?.id ?? "" }, buttons: ["go_minus", "go", "flash"], fader: "master" };
  if (type === "group") return { ...playback, target: { type, group_id: server.groups[0]?.id ?? "" }, buttons: ["select", "flash", "select_contents"], fader: "master" };
  if (type === "speed_group") return { ...playback, target: { type, group: "A" }, buttons: ["double", "half", "learn"], fader: "learned_percentage" };
  if (type === "programmer_fade") return { ...playback, target: { type }, buttons: ["none", "none", "none"], fader: "master" };
  if (type === "cue_fade") return { ...playback, target: { type }, buttons: ["none", "none", "none"], fader: "master" };
  return { ...playback, target: { type: "grand_master" }, buttons: ["blackout", "flash", "pause_dynamics"], fader: "master" };
}
function layoutActions(playback: PlaybackDefinition): PlaybackButtonAction[] { if (playback.target.type === "cue_list") return cueActions; if (playback.target.type === "speed_group") return ["double", "half", "learn", "pause", "none"]; if (playback.target.type === "group") return ["select", "flash", "select_contents", "none"]; if (playback.target.type === "grand_master") return ["blackout", "flash", "pause_dynamics", "none"]; return ["none"]; }
function faderModes(playback: PlaybackDefinition): PlaybackDefinition["fader"][] { if (playback.target.type === "cue_list") return ["master", "x_fade", "temp"]; if (playback.target.type === "speed_group") return ["learned_percentage", "direct_bpm", "centered_relative"]; return ["master"]; }
