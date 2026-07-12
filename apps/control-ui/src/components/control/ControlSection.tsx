import { CommandLineBar } from "./CommandLineBar";
import { ControlLeftPane } from "./ControlLeftPane";
import { ControlRightPane } from "./ControlRightPane";
import { useApp } from "../../state/AppContext";

export function ControlSection() {
  const { state } = useApp();
  return <section className={`control-section ${state.controlMode}`}><CommandLineBar /><ControlLeftPane /><ControlRightPane /></section>;
}
