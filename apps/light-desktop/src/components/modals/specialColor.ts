import type { ProgrammerValuesMutation } from "../../features/programmerValues/contracts";

export type PickerColor = { hue: number; saturation: number; brightness: number };

/** Picker-space HSV→RGB for the dialog swatch; value resolution happens server-side. */
export function hsvToRgb({ hue, saturation, brightness }: PickerColor) {
  const i = Math.floor(hue * 6);
  const f = hue * 6 - i;
  const p = brightness * (1 - saturation);
  const q = brightness * (1 - f * saturation);
  const t = brightness * (1 - (1 - f) * saturation);
  return (
    [
      [brightness, t, p],
      [q, brightness, p],
      [p, brightness, t],
      [p, q, brightness],
      [t, p, brightness],
      [brightness, p, q],
    ] as number[][]
  )[i % 6];
}

/** Release points this close to the gesture start read as "back on the start color". */
const CLOSED_LOOP_EPSILON = 0.02;

/**
 * One server-side color-range fan-out for the ordered selection. `hueTravel` is the signed
 * hue distance accumulated along the drag in revolutions, so ranges may run the long way
 * around the wheel or wind multiple full rainbow cycles. A drag released back on its start
 * color with whole-revolution travel snaps to the exact closed loop, which the server
 * distributes across the selection without repeating the start hue.
 */
export function colorRangeMutation(
  fixtureIds: readonly string[],
  start: PickerColor,
  end: PickerColor,
  hueTravel: number,
  brightness: number,
  fadeMillis?: number,
): ProgrammerValuesMutation {
  const revolutions = Math.round(hueTravel);
  const closed =
    revolutions !== 0 &&
    Math.abs(hueTravel - revolutions) < CLOSED_LOOP_EPSILON &&
    Math.abs(end.hue - start.hue) < CLOSED_LOOP_EPSILON &&
    Math.abs(end.saturation - start.saturation) < CLOSED_LOOP_EPSILON;
  return {
    action: "set_selection_color_range",
    fixtureIds,
    start: { hue: start.hue, saturation: start.saturation },
    end: closed
      ? { hue: start.hue, saturation: start.saturation }
      : { hue: end.hue, saturation: end.saturation },
    hueTravel: closed ? revolutions : hueTravel,
    brightness,
    timing: {
      fade: true,
      fadeMillis: fadeMillis ?? 3_000,
      delayMillis: null,
    },
  };
}
