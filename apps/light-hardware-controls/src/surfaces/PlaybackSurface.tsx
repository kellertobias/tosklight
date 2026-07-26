import {
  attachedPlaybackLayout,
  controlSurfaceOscPaths,
} from "@tosklight/ui/control-surface-contracts";
import { ControlButton } from "../components/ControlButton";
import type { Lamp, SendControl } from "../controller/types";
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
        {attachedPlaybackLayout.encoderSlots.map((number) => (
          <EncoderEmulation key={number} number={number} send={send} />
        ))}
        <EncoderEmulation
          number={attachedPlaybackLayout.navigationEncoder}
          nav
          send={send}
        />
      </div>
      <div className="top-row">
        {attachedPlaybackLayout.topSlots.map((slot) => (
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
      <div className="playback-bank">
        {attachedPlaybackLayout.mainSlots.map((slot) => (
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
