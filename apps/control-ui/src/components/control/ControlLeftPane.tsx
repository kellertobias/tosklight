import { useApp } from "../../state/AppContext";
import { ParameterControls } from "./ParameterControls";
import { PlaybackFaderBank } from "./PlaybackFaderBank";

export function ControlLeftPane() {
  const { state } = useApp();
  return <div className="control-left-pane">{state.controlMode === "programmer" ? <ParameterControls /> : <PlaybackFaderBank />}</div>;
}
