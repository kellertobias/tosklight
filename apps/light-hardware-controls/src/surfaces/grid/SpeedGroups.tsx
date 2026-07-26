import {
  attachedPlaybackLayout,
  controlSurfaceOscPaths,
} from "@tosklight/ui/control-surface-contracts";
import { ControlButton } from "../../components/ControlButton";
import { TouchFader } from "../../components/TouchFader";
import type { Lamp, SendControl } from "../../controller/types";

interface SpeedGroupsProps {
  speedBpms: Record<number, number>;
  lamps: Record<string, Lamp>;
  send: SendControl;
}

export function SpeedGroups({ speedBpms, lamps, send }: SpeedGroupsProps) {
  return (
    <section className="speed-groups">
      <h2>Speed groups</h2>
      {attachedPlaybackLayout.speedGroups.map((number) => {
        const bpm = speedBpms[number] ?? 120;
        return (
          <div className="encoder" key={number}>
            <ControlButton
              label={`SPEED ${number}`}
              lamp={lamps[`speed/${number}`]}
              onDown={() =>
                send(controlSurfaceOscPaths.speedGroupButton(number), [true])
              }
              onUp={() =>
                send(controlSurfaceOscPaths.speedGroupButton(number), [false])
              }
            />
            <TouchFader
              className="speed-touch-fader"
              label="RATE"
              value={(bpm - 1) / 998}
              display={`${bpm} BPM`}
              onChange={(value) => {
                send(controlSurfaceOscPaths.speedGroupEncoder(number), [
                  Math.round(1 + value * 998),
                ]);
              }}
            />
          </div>
        );
      })}
    </section>
  );
}
