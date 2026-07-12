import { useApp } from "../../state/AppContext";
import type { BuiltInWindow } from "../../types";

const choices: Array<[BuiltInWindow, string]> = [["presets", "Preset pool"], ["groups", "Group pool"], ["fixtures", "Fixture sheet"], ["stage", "Stage"], ["playback", "Playback"], ["channels", "Channels"], ["dynamics", "Dynamics"], ["dmx", "DMX output"]];
export function WindowPicker() {
  const { state, dispatch } = useApp();
  if (!state.windowPicker) return null;
  return <div className="floating-dialog window-picker"><h2>Open Window</h2><div className="dialog-grid">{choices.map(([kind, label]) => <button key={kind} onClick={() => dispatch({ type: "ADD_WINDOW", kind })}>{label}</button>)}</div><button className="dialog-done" onClick={() => dispatch({ type: "OPEN_WINDOW_PICKER", rect: null })}>Cancel</button></div>;
}
