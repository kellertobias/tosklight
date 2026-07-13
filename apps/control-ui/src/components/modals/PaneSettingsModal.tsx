import { useApp } from "../../state/AppContext";
import { GRID_COLUMNS, GRID_ROWS } from "../../types";
import { TouchSelect } from "../common/TouchSelect";
import { Button, Input, Select } from "../common";
import { WindowSettings, type WindowSettingsTab } from "../window-kit";

export function PaneSettingsModal() {
  const { state, dispatch } = useApp();
  if (!state.paneSettingsId) return null;
  const desk = state.desks.find((item) => item.id === state.activeDeskId)!;
  const pane = desk.panes.find((item) => item.id === state.paneSettingsId);
  if (!pane) return null;
  const close = () => dispatch({ type: "SET_PANE_SETTINGS", id: null });
  const tabs: WindowSettingsTab[] = [{ id: "pane", label: "Pane Settings", content: <><p>Selected pane: <b>{pane.title}</b></p><div className="size-grid"><TouchSelect label="Grid width" value={pane.width} options={Array.from({ length: GRID_COLUMNS }, (_, index) => index + 1)} onChange={(width) => dispatch({ type: "SET_PANE_RECT", id: pane.id, rect: { width } })}/><TouchSelect label="Grid height" value={pane.height} options={Array.from({ length: GRID_ROWS }, (_, index) => index + 1)} onChange={(height) => dispatch({ type: "SET_PANE_RECT", id: pane.id, rect: { height } })}/></div><div className="dialog-grid"><Button className="danger" onClick={() => dispatch({ type: "REMOVE_PANE", id: pane.id })}>Remove pane</Button></div></> }];
  if (pane.kind === "presets") tabs.push({ id: "pool", label: "Pool", content: <><h3>Preset family</h3><div className="button-group">{(["All", "Intensity", "Color", "Position", "Beam"] as const).map((family) => <Button key={family} className={(pane.presetFamily ?? state.presetFamily) === family ? "active" : ""} onClick={() => dispatch({ type: "SET_PANE_PRESET_FAMILY", id: pane.id, family })}>{family}</Button>)}</div><label className="pane-option-toggle"><Input type="checkbox" checked={pane.presetPoolColors ?? true} onChange={(event) => dispatch({ type: "SET_PANE_PRESET_COLORS", id: pane.id, value: event.target.checked })}/> Enable pool colors</label></> });
  if (pane.kind === "stage") tabs.push({ id: "stage", label: "Stage", content: <><label className="pane-option-toggle">Stage view <Select value={pane.stageView ?? "2d"} onChange={(event) => dispatch({ type: "SET_PANE_STAGE_OPTION", id: pane.id, option: "stageView", value: event.target.value as "2d" | "3d" })}><option value="2d">2D</option><option value="3d">3D</option></Select></label><label className="pane-option-toggle"><Input type="checkbox" checked={Boolean(pane.followPreload)} onChange={(event) => dispatch({ type: "SET_PANE_STAGE_OPTION", id: pane.id, option: "followPreload", value: event.target.checked })}/> Follow Preload</label></> });
  if (["stage", "fixtures", "presets"].includes(pane.kind)) tabs.push({ id: "shortcuts", label: "Shortcuts", content: <label className="pane-option-toggle"><Input type="checkbox" checked={Boolean(pane.showGroupShortcuts)} onChange={(event) => dispatch({ type: "SET_PANE_GROUP_SHORTCUTS", id: pane.id, value: event.target.checked })}/> Show group shortcuts</label> });
  return <WindowSettings title="Pane Settings" tabs={tabs} onClose={close} />;
}
