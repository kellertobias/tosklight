import { useApp } from "../../state/AppContext";
import { NumericPad } from "./NumericPad";
import { PlaybackTools } from "./PlaybackTools";

export function ControlRightPane() {
  const { state } = useApp();
  return <aside className="control-right-pane">{state.controlMode === "programmer" ? <NumericPad /> : <PlaybackTools />}</aside>;
}
