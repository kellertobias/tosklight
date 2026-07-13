import { useApp } from "../../state/AppContext";
import { GRID_COLUMNS, GRID_ROWS } from "../../types";
import { TouchSelect } from "../common/TouchSelect";
import { Button, Input, Select } from "../common";

export function PaneSettingsModal() {
  const { state, dispatch } = useApp();
  if (!state.paneSettingsId) return null;
  const desk = state.desks.find((item) => item.id === state.activeDeskId)!;
  const pane = desk.panes.find((item) => item.id === state.paneSettingsId);
  if (!pane) return null;
  const close = () => dispatch({ type: "SET_PANE_SETTINGS", id: null });
  return <div className="pane-settings-backdrop" onPointerDown={(event) => { if (event.target === event.currentTarget) close(); }}><div className="floating-dialog pane-settings"><Button className="modal-close" onClick={close}>×</Button><h2>Pane Settings</h2><p>Selected pane: <b>{pane.title}</b></p><div className="size-grid"><TouchSelect label="Grid width" value={pane.width} options={Array.from({ length: GRID_COLUMNS }, (_, index) => index + 1)} onChange={(width) => dispatch({ type: "SET_PANE_RECT", id: pane.id, rect: { width } })}/><TouchSelect label="Grid height" value={pane.height} options={Array.from({ length: GRID_ROWS }, (_, index) => index + 1)} onChange={(height) => dispatch({ type: "SET_PANE_RECT", id: pane.id, rect: { height } })}/></div>{["stage", "fixtures", "presets"].includes(pane.kind) && <label className="pane-option-toggle"><Input type="checkbox" checked={Boolean(pane.showGroupShortcuts)} onChange={(event) => dispatch({ type: "SET_PANE_GROUP_SHORTCUTS", id: pane.id, value: event.target.checked })}/> Show group shortcuts</label>}{pane.kind === "stage" && <><label className="pane-option-toggle">Stage view <Select value={pane.stageView ?? "2d"} onChange={(event) => dispatch({ type: "SET_PANE_STAGE_OPTION", id: pane.id, option: "stageView", value: event.target.value as "2d" | "3d" })}><option value="2d">2D</option><option value="3d">3D</option></Select></label><label className="pane-option-toggle"><Input type="checkbox" checked={Boolean(pane.followPreload)} onChange={(event) => dispatch({ type: "SET_PANE_STAGE_OPTION", id: pane.id, option: "followPreload", value: event.target.checked })}/> Follow Preload</label></>}<div className="dialog-grid"><Button className="danger" onClick={() => dispatch({ type: "REMOVE_PANE", id: pane.id })}>Remove pane</Button></div><Button className="dialog-done" onClick={close}>Done</Button><small>Drag the window title to move this pane by whole grid tiles.</small></div></div>;
}
