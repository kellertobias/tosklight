import { CommandLineBar } from "./CommandLineBar";
import { ControlLeftPane } from "./ControlLeftPane";
import { ControlRightPane } from "./ControlRightPane";
import { useApp } from "../../state/AppContext";
import { useServer } from "../../api/ServerContext";

export function ControlSection() {
  const { state } = useApp();
  const server = useServer();
  const hardware = Boolean(server.bootstrap?.hardware_connected || state.midiProfile);
  return <section className={`control-section ${state.controlMode} ${hardware ? "hardware-connected" : "touch-connected"}`}><CommandLineBar /><ControlLeftPane /><ControlRightPane /></section>;
}
