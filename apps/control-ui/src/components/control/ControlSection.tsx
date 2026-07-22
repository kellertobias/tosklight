import { CommandLineBar } from "./CommandLineBar";
import { ControlLeftPane } from "./ControlLeftPane";
import { ControlRightPane } from "./ControlRightPane";
import { useApp } from "../../state/AppContext";
import { useHardwareConnected } from "../../features/deskSnapshot/DeskSnapshotState";

export function ControlSection() {
  const { state } = useApp();
  const hardwareConnected = useHardwareConnected();
  const hardware = Boolean(hardwareConnected || state.midiProfile);
  return <section className={`control-section ${state.controlMode} ${hardware ? "hardware-connected" : "touch-connected"}`}><CommandLineBar /><ControlLeftPane /><ControlRightPane /></section>;
}
