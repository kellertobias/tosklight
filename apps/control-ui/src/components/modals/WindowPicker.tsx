import { useApp } from "../../state/AppContext";
import type { BuiltInWindow } from "../../types";
import { Button } from "../common";

export const windowChoices: Array<[BuiltInWindow, string]> = [["presets", "Preset pool"], ["groups", "Group pool"], ["fixtures", "Fixture sheet"], ["stage", "Stage"], ["cuelist_pool", "Cuelist Pool"], ["cues", "Cues · Cuelist"], ["cuelists", "Cuelists (tabs)"], ["virtual_playbacks", "Virtual Playbacks"], ["channels", "Channels"], ["dynamics", "Dynamics"], ["dmx", "DMX output"], ["help", "Help"], ["development", "Development"]];
export function WindowPicker() {
  const { state, dispatch } = useApp();
  if (!state.windowPicker) return null;
  return <div className="floating-dialog window-picker"><h2>Open Window</h2><div className="dialog-grid">{windowChoices.map(([kind, label]) => <Button key={kind} onClick={() => dispatch({ type: "ADD_WINDOW", kind })}>{label}</Button>)}</div><Button className="dialog-done" onClick={() => dispatch({ type: "OPEN_WINDOW_PICKER", rect: null })}>Cancel</Button></div>;
}
