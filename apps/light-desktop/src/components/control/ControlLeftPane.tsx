import { useApp } from "../../state/AppContext";
import {
  useHardwareConnected,
  useSessionSnapshot,
} from "../../features/deskSnapshot/DeskSnapshotState";
import { ParameterControls } from "./ParameterControls";
import { PlaybackFaderBank } from "./PlaybackFaderBank";
import { PatchParameterControls } from "./PatchParameterControls";

export function ControlLeftPane() {
  const { state } = useApp();
  const session = useSessionSnapshot();
  const hardwareConnected = useHardwareConnected();
  return <div className="control-left-pane">{state.controlMode === "programmer" ? state.builtIn === "patch" ? <PatchParameterControls /> : <ParameterControls /> : <PlaybackFaderBank playbackLayout={session?.desk.playback_layout} hardwareConnected={hardwareConnected} />}</div>;
}
