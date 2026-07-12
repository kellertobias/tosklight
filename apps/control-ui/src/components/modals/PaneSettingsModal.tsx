import { useApp } from "../../state/AppContext";
import { GRID_COLUMNS, GRID_ROWS } from "../../types";

export function PaneSettingsModal() {
  const { state, dispatch } = useApp();
  if (!state.paneSettingsId) return null;
  const desk = state.desks.find((item) => item.id === state.activeDeskId)!;
  const pane = desk.panes.find((item) => item.id === state.paneSettingsId);
  if (!pane) return null;
  return <div className="floating-dialog pane-settings"><h2>Pane Settings</h2><p>Selected pane: <b>{pane.title}</b></p><div className="size-grid"><label>Grid width<select value={pane.width} onChange={(event) => dispatch({ type: "SET_PANE_RECT", id: pane.id, rect: { width: Number(event.target.value) } })}>{Array.from({ length: GRID_COLUMNS }, (_, index) => <option key={index + 1}>{index + 1}</option>)}</select></label><label>Grid height<select value={pane.height} onChange={(event) => dispatch({ type: "SET_PANE_RECT", id: pane.id, rect: { height: Number(event.target.value) } })}>{Array.from({ length: GRID_ROWS }, (_, index) => <option key={index + 1}>{index + 1}</option>)}</select></label></div><div className="dialog-grid"><button onClick={() => dispatch({ type: "TOGGLE_MAXIMIZE", id: pane.id })}>Full screen</button><button className="danger" onClick={() => dispatch({ type: "REMOVE_PANE", id: pane.id })}>Remove pane</button></div><button className="dialog-done" onClick={() => dispatch({ type: "SET_PANE_SETTINGS", id: null })}>Done</button><small>Pane content is selected when opening the window and cannot be changed here.</small></div>;
}
