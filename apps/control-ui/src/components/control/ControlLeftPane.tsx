import { useApp } from "../../state/AppContext";
import { ParameterControls } from "./ParameterControls";
import { PlaybackFaderBank } from "./PlaybackFaderBank";
import { PatchParameterControls } from "./PatchParameterControls";

export function ControlLeftPane() {
  const { state } = useApp();
  return <div className="control-left-pane">{state.controlMode === "programmer" ? state.builtIn === "patch" ? <PatchParameterControls /> : <ParameterControls /> : <PlaybackFaderBank />}</div>;
}
