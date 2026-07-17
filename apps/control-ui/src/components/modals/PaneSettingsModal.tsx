import { useState } from "react";
import { useServer } from "../../api/ServerContext";
import { useApp } from "../../state/AppContext";
import { GRID_COLUMNS, GRID_ROWS, type PaneModel, type VirtualPlaybackExclusionZone } from "../../types";
import { TouchSelect } from "../common/TouchSelect";
import { Button, FormLayout, MultiValueToggleField, NumberField, SelectField, SwitchField, TextField } from "../common";
import { WindowSettings, type WindowSettingsTab } from "../window-kit";
import { DEVELOPMENT_VIEW_OPTIONS } from "../../windows/DevelopmentWindow";
import { requestPaneRemoval } from "../shell/paneRemovalGuard";

function VirtualPlaybackZoneEditor({ pane, zone, zones, visibleCells }: { pane: PaneModel; zone: VirtualPlaybackExclusionZone; zones: VirtualPlaybackExclusionZone[]; visibleCells: number }) {
  const { dispatch } = useApp();
  const server = useServer();
  const [name, setName] = useState(zone.name);
  const persist = async (next: VirtualPlaybackExclusionZone[]) => {
    if (await server.saveVirtualPlaybackExclusionZones(pane.id, next)) dispatch({ type: "SET_VIRTUAL_PLAYBACK_EXCLUSION_ZONES", id: pane.id, zones: next });
  };
  const saveName = () => {
    const trimmed = name.trim();
    if (!trimmed || trimmed === zone.name) return;
    void persist(zones.map((candidate) => candidate.id === zone.id ? { ...candidate, name: trimmed } : candidate));
  };
  const toggleSlot = (slot: number) => {
    const slots = zone.slots.includes(slot) ? zone.slots.filter((candidate) => candidate !== slot) : [...zone.slots, slot].sort((left, right) => left - right);
    if (slots.length < 2) return;
    void persist(zones.map((candidate) => candidate.id === zone.id ? { ...candidate, slots } : candidate));
  };
  const hiddenSlots = zone.slots.filter((slot) => slot > visibleCells);
  return <article className="virtual-playback-zone-editor">
    <header><TextField label={`Name for ${zone.name}`} maxLength={80} value={name} onChange={(event) => setName(event.target.value)}/><Button disabled={!name.trim() || name.trim() === zone.name} onClick={saveName}>Save name</Button><Button className="danger" onClick={() => void persist(zones.filter((candidate) => candidate.id !== zone.id))}>Delete zone</Button></header>
    <div className="virtual-playback-zone-members" role="group" aria-label={`${zone.name} cells`}>
      {Array.from({ length: visibleCells }, (_, index) => index + 1).map((slot) => <Button key={slot} active={zone.slots.includes(slot)} aria-label={`${zone.name} cell ${slot}`} onClick={() => toggleSlot(slot)}>{slot}</Button>)}
    </div>
    {hiddenSlots.length > 0 && <div className="virtual-playback-zone-hidden"><small>{hiddenSlots.length} hidden grid {hiddenSlots.length === 1 ? "cell is" : "cells are"} retained:</small>{hiddenSlots.map((slot) => <Button key={slot} active aria-label={`${zone.name} hidden cell ${slot}`} onClick={() => toggleSlot(slot)}>{slot}</Button>)}</div>}
    <small>{zone.slots.length} cells · zone order {zones.findIndex((candidate) => candidate.id === zone.id) + 1}</small>
  </article>;
}

function VirtualPlaybackZoneSettings({ pane, rows, columns }: { pane: PaneModel; rows: number; columns: number }) {
  const zones = pane.virtualPlaybackExclusionZones ?? [];
  return <section className="virtual-playback-zone-settings" aria-label="Playback Exclusion Zones"><h3>Playback Exclusion Zones</h3><p>Shift-select at least two cells in the pane to create a zone. A newly activated member releases the other active members; creating or editing a zone never operates a playback.</p>{zones.length === 0 ? <p>No exclusion zones are configured for this pane.</p> : zones.map((zone) => <VirtualPlaybackZoneEditor key={zone.id} pane={pane} zone={zone} zones={zones} visibleCells={rows * columns}/>)}</section>;
}

export function PaneSettingsModal() {
  const { state, dispatch } = useApp();
  if (!state.paneSettingsId) return null;
  const desk = state.desks.find((item) => item.id === state.activeDeskId)!;
  const pane = desk.panes.find((item) => item.id === state.paneSettingsId);
  if (!pane) return null;
  const close = () => dispatch({ type: "SET_PANE_SETTINGS", id: null });
  const tabs: WindowSettingsTab[] = [{ id: "pane", label: "Pane Settings", content: <><p>Selected pane: <b>{pane.title}</b></p><div className="size-grid"><TouchSelect label="Grid width" value={pane.width} options={Array.from({ length: GRID_COLUMNS }, (_, index) => index + 1)} onChange={(width) => dispatch({ type: "SET_PANE_RECT", id: pane.id, rect: { width } })}/><TouchSelect label="Grid height" value={pane.height} options={Array.from({ length: GRID_ROWS }, (_, index) => index + 1)} onChange={(height) => dispatch({ type: "SET_PANE_RECT", id: pane.id, rect: { height } })}/></div><div className="dialog-grid"><Button className="danger" onClick={() => { if (requestPaneRemoval(pane.id)) dispatch({ type: "REMOVE_PANE", id: pane.id }); }}>Remove pane</Button></div></> }];
  if (pane.kind === "presets") tabs.push({ id: "pool", label: "Pool", content: <><h3>Preset family</h3><div className="button-group">{(["All", "Intensity", "Color", "Position", "Beam"] as const).map((family) => <Button key={family} className={(pane.presetFamily ?? state.presetFamily) === family ? "active" : ""} onClick={() => dispatch({ type: "SET_PANE_PRESET_FAMILY", id: pane.id, family })}>{family}</Button>)}</div><SwitchField label="Enable pool colors" checked={pane.presetPoolColors ?? true} onChange={(event) => dispatch({ type: "SET_PANE_PRESET_COLORS", id: pane.id, value: event.target.checked })}/></> });
  if (pane.kind === "stage") tabs.push({ id: "stage", label: "Stage", content: <FormLayout labelPlacement="side"><MultiValueToggleField label="Stage view" value={pane.stageView ?? "2d"} onChange={(value) => dispatch({ type: "SET_PANE_STAGE_OPTION", id: pane.id, option: "stageView", value })} options={[{ value: "2d", label: "2D" }, { value: "3d", label: "3D" }]}/><SwitchField label="Follow Preload" checked={Boolean(pane.followPreload)} onChange={(event) => dispatch({ type: "SET_PANE_STAGE_OPTION", id: pane.id, option: "followPreload", value: event.target.checked })}/></FormLayout> });
  if (pane.kind === "development") tabs.push({ id: "development", label: "Development", content: <FormLayout labelPlacement="side"><SelectField label="Shown example" value={pane.developmentView ?? "forms"} onChange={(value) => dispatch({ type: "SET_PANE_DEVELOPMENT_VIEW", id: pane.id, value })} options={DEVELOPMENT_VIEW_OPTIONS}/></FormLayout> });
  if (pane.kind === "virtual_playbacks") {
    const rows = pane.virtualPlaybackRows ?? 2;
    const columns = pane.virtualPlaybackColumns ?? 2;
    tabs.push({ id: "virtual", label: "Virtual Playbacks", content: <><FormLayout labelPlacement="side"><NumberField label="Rows" min="1" max="12" value={rows} onChange={(event) => dispatch({ type: "SET_VIRTUAL_PLAYBACK_GRID", id: pane.id, rows: Number(event.target.value), columns })}/><NumberField label="Columns" min="1" max="12" value={columns} onChange={(event) => dispatch({ type: "SET_VIRTUAL_PLAYBACK_GRID", id: pane.id, rows, columns: Number(event.target.value) })}/></FormLayout><p>Assign cells with <b>Set Source</b>, <b>Add Target</b>, or the normal [SET], source, target sequence.</p><VirtualPlaybackZoneSettings pane={pane} rows={rows} columns={columns}/></> });
  }
  if (pane.kind === "file_manager") tabs.push({ id: "files", label: "File Manager", content: <FormLayout labelPlacement="side"><SwitchField label="Show Hidden" checked={Boolean(pane.fileManagerShowHidden)} onChange={(event) => dispatch({ type: "SET_FILE_MANAGER_SHOW_HIDDEN", id: pane.id, value: event.target.checked })}/></FormLayout> });
  if (pane.kind === "text_editor") tabs.push({ id: "editor", label: "Text Editor", content: <FormLayout labelPlacement="side"><SwitchField label="Read-only pane" checked={Boolean(pane.textEditorReadOnly)} onChange={(event) => dispatch({ type: "SET_TEXT_EDITOR_SETTINGS", id: pane.id, readOnly: event.target.checked })}/><MultiValueToggleField label="View" value={pane.textEditorMode ?? "plain"} onChange={(mode) => dispatch({ type: "SET_TEXT_EDITOR_SETTINGS", id: pane.id, mode })} options={[{ value: "plain", label: "Plain Text" }, { value: "markdown", label: "Rendered Markdown" }, { value: "split", label: "Edit + Markdown" }]}/></FormLayout> });
  if (["stage", "fixtures", "presets"].includes(pane.kind)) tabs.push({ id: "shortcuts", label: "Shortcuts", content: <SwitchField label="Show group shortcuts" checked={Boolean(pane.showGroupShortcuts)} onChange={(event) => dispatch({ type: "SET_PANE_GROUP_SHORTCUTS", id: pane.id, value: event.target.checked })}/> });
  return <WindowSettings title="Pane Settings" tabs={tabs} onClose={close} />;
}
