import {
  attachedPlaybackLayout,
  controlSurfaceOscPaths,
} from "@tosklight/ui/control-surface-contracts";
import { ControlButton } from "../components/ControlButton";
import type { Lamp, SendControl } from "../controller/types";
import { SpeedGroups } from "./grid/SpeedGroups";
import { Playback } from "./playback/Playback";

interface GridSurfaceProps {
  levels: Record<number, number>;
  lamps: Record<string, Lamp>;
  speedBpms: Record<number, number>;
  send: SendControl;
}

export function GridSurface({
  levels,
  lamps,
  speedBpms,
  send,
}: GridSurfaceProps) {
  return (
    <section className="grid-layout">
      <div className="button-grid">
        {attachedPlaybackLayout.gridButtonSlots.map((slot) => (
          <ControlButton
            key={slot}
            label={String(slot)}
            lamp={lamps[`${slot}/1`]}
            onDown={() =>
              send(
                controlSurfaceOscPaths.pagePlaybackControl(slot, "button/1"),
                [true],
              )
            }
            onUp={() =>
              send(
                controlSurfaceOscPaths.pagePlaybackControl(slot, "button/1"),
                [false],
              )
            }
          />
        ))}
      </div>
      <aside className="grid-sidebar">
        <section className="six">
          <h2>Playbacks 91–96</h2>
          {attachedPlaybackLayout.gridPlaybackSlots.map((slot) => (
            <Playback
              key={slot}
              slot={slot}
              buttons={1}
              levels={levels}
              lamps={lamps}
              send={send}
            />
          ))}
        </section>
        <SpeedGroups speedBpms={speedBpms} lamps={lamps} send={send} />
      </aside>
    </section>
  );
}
