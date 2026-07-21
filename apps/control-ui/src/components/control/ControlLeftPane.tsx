import { useApp } from "../../state/AppContext";
import { useServer } from "../../api/ServerContext";
import { ParameterControls } from "./ParameterControls";
import { PlaybackFaderBank } from "./PlaybackFaderBank";
import { PatchParameterControls } from "./PatchParameterControls";

export function ControlLeftPane() {
  const { state } = useApp();
  const server = useServer();
  return <div className="control-left-pane">{state.controlMode === "programmer" ? state.builtIn === "patch" ? <PatchParameterControls /> : <ParameterControls /> : <PlaybackFaderBank playbackLayout={server.session?.desk.playback_layout} hardwareConnected={Boolean(server.bootstrap?.hardware_connected)} />}</div>;
}
