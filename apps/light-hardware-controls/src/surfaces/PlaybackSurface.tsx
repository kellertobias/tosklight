import { ControlButton } from "../components/ControlButton";
import type { Lamp, SendControl } from "../controller/types";
import { oscPaths } from "../oscPaths";
import { EncoderEmulation } from "./playback/EncoderEmulation";
import { Playback } from "./playback/Playback";

interface PlaybackSurfaceProps {
  topRowVisible: boolean;
  levels: Record<number, number>;
  lamps: Record<string, Lamp>;
  send: SendControl;
}

export function PlaybackSurface({
  topRowVisible,
  levels,
  lamps,
  send,
}: PlaybackSurfaceProps) {
  return (
    <section
      className={`playback-surface ${topRowVisible ? "with-top-row" : "without-top-row"}`}
    >
      <div className="encoder-row">
        {Array.from({ length: 6 }, (_, index) => (
          <EncoderEmulation key={index} number={index + 1} send={send} />
        ))}
        <EncoderEmulation number={7} nav send={send} />
      </div>
      <div className="top-row">
        {Array.from({ length: 20 }, (_, index) => index + 21).map((slot) => (
          <ControlButton
            key={slot}
            label={String(slot)}
            lamp={lamps[`${slot}/1`]}
            onDown={() => send(`${oscPaths.pagePlayback(slot)}/button/1`, [true])}
            onUp={() => send(`${oscPaths.pagePlayback(slot)}/button/1`, [false])}
          />
        ))}
      </div>
      <div className="playback-bank">
        {Array.from({ length: 20 }, (_, index) => index + 1).map((slot) => (
          <Playback
            key={slot}
            slot={slot}
            levels={levels}
            lamps={lamps}
            send={send}
          />
        ))}
      </div>
    </section>
  );
}
