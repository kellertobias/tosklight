import { useApp } from "../../state/AppContext";
import { NumericPad } from "./NumericPad";
import { PlaybackTools } from "./PlaybackTools";
import { useHardwareConnected } from "../../features/deskSnapshot/DeskSnapshotState";
import { HardwareControlSummary } from "./HardwareControlSummary";

export function ControlRightPane() {
  const { state } = useApp();
  const hardwareConnected = useHardwareConnected();
  if (hardwareConnected || state.midiProfile) return <aside className="control-right-pane hardware-right-pane"><HardwareControlSummary /></aside>;
  if (state.controlMode === "programmer") return <aside className="control-right-pane"><div className="control-right-main"><NumericPad /></div></aside>;
  return <aside className="control-right-pane"><div className="control-right-main"><PlaybackTools /></div></aside>;
}
