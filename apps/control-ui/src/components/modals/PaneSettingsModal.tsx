import { useApp } from "../../state/AppContext";
import { GRID_COLUMNS, GRID_ROWS } from "../../types";
import { TouchSelect } from "../common/TouchSelect";
import { Button, FormLayout, MultiValueToggleField, NumberField, SelectField, SwitchField } from "../common";
import { WindowSettings, type WindowSettingsTab } from "../window-kit";
import { DEVELOPMENT_VIEW_OPTIONS } from "../../windows/DevelopmentWindow";
import { useServer } from "../../api/ServerContext";

export function PaneSettingsModal() {
  const { state, dispatch } = useApp();
  const server = useServer();
  if (!state.paneSettingsId) return null;
  const desk = state.desks.find((item) => item.id === state.activeDeskId)!;
  const pane = desk.panes.find((item) => item.id === state.paneSettingsId);
  if (!pane) return null;
  const close = () => dispatch({ type: "SET_PANE_SETTINGS", id: null });
  const tabs: WindowSettingsTab[] = [{ id: "pane", label: "Pane Settings", content: <><p>Selected pane: <b>{pane.title}</b></p><div className="size-grid"><TouchSelect label="Grid width" value={pane.width} options={Array.from({ length: GRID_COLUMNS }, (_, index) => index + 1)} onChange={(width) => dispatch({ type: "SET_PANE_RECT", id: pane.id, rect: { width } })}/><TouchSelect label="Grid height" value={pane.height} options={Array.from({ length: GRID_ROWS }, (_, index) => index + 1)} onChange={(height) => dispatch({ type: "SET_PANE_RECT", id: pane.id, rect: { height } })}/></div><div className="dialog-grid"><Button className="danger" onClick={() => dispatch({ type: "REMOVE_PANE", id: pane.id })}>Remove pane</Button></div></> }];
  if (pane.kind === "presets") tabs.push({ id: "pool", label: "Pool", content: <><h3>Preset family</h3><div className="button-group">{(["All", "Intensity", "Color", "Position", "Beam"] as const).map((family) => <Button key={family} className={(pane.presetFamily ?? state.presetFamily) === family ? "active" : ""} onClick={() => dispatch({ type: "SET_PANE_PRESET_FAMILY", id: pane.id, family })}>{family}</Button>)}</div><SwitchField label="Enable pool colors" checked={pane.presetPoolColors ?? true} onChange={(event) => dispatch({ type: "SET_PANE_PRESET_COLORS", id: pane.id, value: event.target.checked })}/></> });
  if (pane.kind === "stage") tabs.push({ id: "stage", label: "Stage", content: <FormLayout labelPlacement="side"><MultiValueToggleField label="Stage view" value={pane.stageView ?? "2d"} onChange={(value) => dispatch({ type: "SET_PANE_STAGE_OPTION", id: pane.id, option: "stageView", value })} options={[{ value: "2d", label: "2D" }, { value: "3d", label: "3D" }]}/><SwitchField label="Follow Preload" checked={Boolean(pane.followPreload)} onChange={(event) => dispatch({ type: "SET_PANE_STAGE_OPTION", id: pane.id, option: "followPreload", value: event.target.checked })}/></FormLayout> });
  if (pane.kind === "development") tabs.push({ id: "development", label: "Development", content: <FormLayout labelPlacement="side"><SelectField label="Shown example" value={pane.developmentView ?? "forms"} onChange={(value) => dispatch({ type: "SET_PANE_DEVELOPMENT_VIEW", id: pane.id, value })} options={DEVELOPMENT_VIEW_OPTIONS}/></FormLayout> });
  if (pane.kind === "virtual_playbacks") {
    const rows = pane.virtualPlaybackRows ?? 2;
    const columns = pane.virtualPlaybackColumns ?? 2;
    tabs.push({ id: "virtual", label: "Virtual Playbacks", content: <><FormLayout labelPlacement="side"><NumberField label="Rows" min="1" max="12" value={rows} onChange={(event) => dispatch({ type: "SET_VIRTUAL_PLAYBACK_GRID", id: pane.id, rows: Number(event.target.value), columns })}/><NumberField label="Columns" min="1" max="12" value={columns} onChange={(event) => dispatch({ type: "SET_VIRTUAL_PLAYBACK_GRID", id: pane.id, rows, columns: Number(event.target.value) })}/></FormLayout>{Array.from({ length: rows * columns }, (_, index) => { const cell = pane.virtualPlaybackCells?.[index] ?? { playbackNumber: null, action: "go" as const }; return <FormLayout key={index} labelPlacement="side"><SelectField label={`Cell ${index + 1} Cuelist`} value={cell.playbackNumber == null ? "" : String(cell.playbackNumber)} onChange={(value) => dispatch({ type: "SET_VIRTUAL_PLAYBACK_CELL", id: pane.id, index, playbackNumber: value ? Number(value) : null })} options={[{ value: "", label: "Unassigned" }, ...(server.playbacks?.pool ?? []).map((playback) => ({ value: String(playback.number), label: `${playback.number} · ${playback.name}` }))]}/><SelectField label={`Cell ${index + 1} action`} value={cell.action} onChange={(action) => dispatch({ type: "SET_VIRTUAL_PLAYBACK_CELL", id: pane.id, index, action: action as "go" | "toggle" })} options={[{ value: "go", label: "GO" }, { value: "toggle", label: "TOGGLE" }]}/></FormLayout>; })}</> });
  }
  if (["stage", "fixtures", "presets"].includes(pane.kind)) tabs.push({ id: "shortcuts", label: "Shortcuts", content: <SwitchField label="Show group shortcuts" checked={Boolean(pane.showGroupShortcuts)} onChange={(event) => dispatch({ type: "SET_PANE_GROUP_SHORTCUTS", id: pane.id, value: event.target.checked })}/> });
  return <WindowSettings title="Pane Settings" tabs={tabs} onClose={close} />;
}
