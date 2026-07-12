import { useApp } from "../../state/AppContext";
import { GRID_COLUMNS, GRID_ROWS } from "../../types";
import { TouchSelect } from "../common/TouchSelect";

export function PaneSettingsModal() {
  const { state, dispatch } = useApp();
  if (!state.paneSettingsId) return null;
  const desk = state.desks.find((item) => item.id === state.activeDeskId)!;
  const pane = desk.panes.find((item) => item.id === state.paneSettingsId);
  if (!pane) return null;
  const close = () => dispatch({ type: "SET_PANE_SETTINGS", id: null });
  return <div className="pane-settings-backdrop" onPointerDown={(event) => { if (event.target === event.currentTarget) close(); }}><div className="floating-dialog pane-settings"><button className="modal-close" onClick={close}>×</button><h2>Pane Settings</h2><p>Selected pane: <b>{pane.title}</b></p><div className="size-grid"><TouchSelect label="Grid width" value={pane.width} options={Array.from({ length: GRID_COLUMNS }, (_, index) => index + 1)} onChange={(width) => dispatch({ type: "SET_PANE_RECT", id: pane.id, rect: { width } })}/><TouchSelect label="Grid height" value={pane.height} options={Array.from({ length: GRID_ROWS }, (_, index) => index + 1)} onChange={(height) => dispatch({ type: "SET_PANE_RECT", id: pane.id, rect: { height } })}/></div>{["stage", "fixtures", "presets"].includes(pane.kind) && <label className="pane-option-toggle"><input type="checkbox" checked={Boolean(pane.showGroupShortcuts)} onChange={(event) => dispatch({ type: "SET_PANE_GROUP_SHORTCUTS", id: pane.id, value: event.target.checked })}/> Show group shortcuts</label>}<div className="dialog-grid"><button className="danger" onClick={() => dispatch({ type: "REMOVE_PANE", id: pane.id })}>Remove pane</button></div><button className="dialog-done" onClick={close}>Done</button><small>Drag the window title to move this pane by whole grid tiles.</small></div></div>;
}
