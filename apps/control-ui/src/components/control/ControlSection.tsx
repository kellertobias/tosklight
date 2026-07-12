import { CommandLineBar } from "./CommandLineBar";
import { ControlContent } from "./ControlContent";

export function ControlSection() {
  return <section className="control-section"><CommandLineBar /><ControlContent /></section>;
}
