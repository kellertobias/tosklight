import { CommandLineBar } from "./CommandLineBar";
import { ControlLeftPane } from "./ControlLeftPane";
import { ControlRightPane } from "./ControlRightPane";
import { useApp } from "../../state/AppContext";
import { useHardwareConnected } from "../../features/deskSnapshot/DeskSnapshotState";

export function ControlSectionView({
  mode,
  hardware,
  commandLine,
  left,
  right,
}: {
  mode: "programmer" | "playbacks";
  hardware: boolean;
  commandLine: React.ReactNode;
  left: React.ReactNode;
  right: React.ReactNode;
}) {
  return <section className={`control-section ${mode} ${hardware ? "hardware-connected" : "touch-connected"}`}>
    {commandLine}
    {left}
    {right}
  </section>;
}

export function ControlSection() {
  const { state } = useApp();
  const hardwareConnected = useHardwareConnected();
  const hardware = Boolean(hardwareConnected || state.midiProfile);
  return <ControlSectionView
    commandLine={<CommandLineBar />}
    hardware={hardware}
    left={<ControlLeftPane />}
    mode={state.controlMode}
    right={<ControlRightPane />}
  />;
}
