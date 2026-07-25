import { ControlButton } from "../components/ControlButton";
import type { Lamp, SendControl } from "../controller/types";
import { oscPaths } from "../oscPaths";
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
        {Array.from({ length: 50 }, (_, index) => index + 41).map((slot) => (
          <ControlButton
            key={slot}
            label={String(slot)}
            lamp={lamps[`${slot}/1`]}
            onDown={() => send(`${oscPaths.pagePlayback(slot)}/button/1`, [true])}
            onUp={() => send(`${oscPaths.pagePlayback(slot)}/button/1`, [false])}
          />
        ))}
      </div>
      <aside className="grid-sidebar">
        <section className="six">
          <h2>Playbacks 91–96</h2>
          {Array.from({ length: 6 }, (_, index) => index + 91).map((slot) => (
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
