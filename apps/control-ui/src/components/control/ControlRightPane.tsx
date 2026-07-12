import { useApp } from "../../state/AppContext";
import { NumericPad } from "./NumericPad";
import { PlaybackTools } from "./PlaybackTools";
import { ProgrammerFadeFader } from "./ProgrammerFadeFader";

export function ControlRightPane() {
  const { state } = useApp();
  return <aside className="control-right-pane"><ProgrammerFadeFader /><div className="control-right-main">{state.controlMode === "programmer" ? <NumericPad /> : <PlaybackTools />}</div></aside>;
}
