import { ControlButton } from "../../components/ControlButton";
import { TouchFader } from "../../components/TouchFader";
import type { Lamp, SendControl } from "../../controller/types";
import { oscPaths } from "../../oscPaths";

interface SpeedGroupsProps {
  speedBpms: Record<number, number>;
  lamps: Record<string, Lamp>;
  send: SendControl;
}

export function SpeedGroups({ speedBpms, lamps, send }: SpeedGroupsProps) {
  return (
    <section className="speed-groups">
      <h2>Speed groups</h2>
      {[1, 2, 3, 4, 5].map((number) => {
        const bpm = speedBpms[number] ?? 120;
        return (
          <div className="encoder" key={number}>
            <ControlButton
              label={`SPEED ${number}`}
              lamp={lamps[`speed/${number}`]}
              onDown={() => send(oscPaths.speedGroupButton(number), [true])}
              onUp={() => send(oscPaths.speedGroupButton(number), [false])}
            />
            <TouchFader
              className="speed-touch-fader"
              label="RATE"
              value={(bpm - 1) / 998}
              display={`${bpm} BPM`}
              onChange={(value) => {
                send(oscPaths.speedGroupEncoder(number), [
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
