import { memo } from "react";
import { ControlButton } from "../../components/ControlButton";
import { TouchFader } from "../../components/TouchFader";
import type { Lamp, SendControl } from "../../controller/types";
import { oscPaths } from "../../oscPaths";

interface PlaybackProps {
  slot: number;
  levels: Record<number, number>;
  lamps: Record<string, Lamp>;
  send: SendControl;
  buttons?: 1 | 3;
}

function PlaybackView({
  slot,
  levels,
  lamps,
  send,
  buttons = 3,
}: PlaybackProps) {
  const base = oscPaths.pagePlayback(slot);
  const button = (number: number, className = "") => (
    <ControlButton
      className={className}
      label={String(number)}
      lamp={lamps[`${slot}/${number}`]}
      onDown={() => send(`${base}/button/${number}`, [true])}
      onUp={() => send(`${base}/button/${number}`, [false])}
    />
  );
  const level = levels[slot] ?? 0;

  return (
    <article className={`playback buttons-${buttons}`}>
      <b>{slot}</b>
      {button(1, "playback-button-one")}
      <TouchFader
        className="playback-touch-fader"
        label="FADER"
        value={level}
        display={`${Math.round(level * 100)}%`}
        onChange={(value) => send(`${base}/fader`, [value])}
      />
      {buttons === 3 && <footer>{button(2)}{button(3)}</footer>}
    </article>
  );
}

function sameLamp(left: Lamp | undefined, right: Lamp | undefined): boolean {
  return left?.color === right?.color
    && left?.state === right?.state
    && left?.bpm === right?.bpm;
}

function samePlayback(previous: PlaybackProps, next: PlaybackProps): boolean {
  if (
    previous.slot !== next.slot
    || previous.buttons !== next.buttons
    || previous.send !== next.send
    || previous.levels[previous.slot] !== next.levels[next.slot]
  ) {
    return false;
  }
  const buttonCount = previous.buttons ?? 3;
  return Array.from({ length: buttonCount }, (_, index) => index + 1).every(
    (button) => sameLamp(
      previous.lamps[`${previous.slot}/${button}`],
      next.lamps[`${next.slot}/${button}`],
    ),
  );
}

export const Playback = memo(PlaybackView, samePlayback);
