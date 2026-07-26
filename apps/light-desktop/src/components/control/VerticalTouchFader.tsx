import { VerticalTouchFaderSurface, type VerticalTouchFaderProps } from "@tosklight/ui/faders";
import { useHardwareConnected } from "../../features/deskSnapshot/DeskSnapshotState";
import { useApp } from "../../state/AppContext";

export function VerticalTouchFader(props: VerticalTouchFaderProps) {
  const hardwareConnected = useHardwareConnected();
  const { state } = useApp();
  return (
    <VerticalTouchFaderSurface
      {...props}
      hardware={Boolean(hardwareConnected || state.midiProfile)}
    />
  );
}
