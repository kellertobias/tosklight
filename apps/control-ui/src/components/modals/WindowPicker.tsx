import { useApp } from "../../state/AppContext";
import type { BuiltInWindow } from "../../types";
import { Button } from "../common";

const choices: Array<[BuiltInWindow, string]> = [["presets", "Preset pool"], ["groups", "Group pool"], ["fixtures", "Fixture sheet"], ["stage", "Stage"], ["playback_pool", "Playback pool"], ["cue_list", "Cue list"], ["playback", "Playback (tabs)"], ["channels", "Channels"], ["dynamics", "Dynamics"], ["dmx", "DMX output"]];
export function WindowPicker() {
  const { state, dispatch } = useApp();
  if (!state.windowPicker) return null;
  return <div className="floating-dialog window-picker"><h2>Open Window</h2><div className="dialog-grid">{choices.map(([kind, label]) => <Button key={kind} onClick={() => dispatch({ type: "ADD_WINDOW", kind })}>{label}</Button>)}</div><Button className="dialog-done" onClick={() => dispatch({ type: "OPEN_WINDOW_PICKER", rect: null })}>Cancel</Button></div>;
}
