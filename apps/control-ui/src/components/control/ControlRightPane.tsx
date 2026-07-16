import { useApp } from "../../state/AppContext";
import { NumericPad } from "./NumericPad";
import { PlaybackTools } from "./PlaybackTools";
import { useServer } from "../../api/ServerContext";
import { HardwareControlSummary } from "./HardwareControlSummary";

export function ControlRightPane() {
  const { state } = useApp();
  const server = useServer();
  if (server.bootstrap?.hardware_connected || state.midiProfile) return <aside className="control-right-pane hardware-right-pane"><HardwareControlSummary /></aside>;
  if (state.controlMode === "programmer") return <aside className="control-right-pane"><div className="control-right-main"><NumericPad /></div></aside>;
  return <aside className="control-right-pane"><div className="control-right-main"><PlaybackTools /></div></aside>;
}
