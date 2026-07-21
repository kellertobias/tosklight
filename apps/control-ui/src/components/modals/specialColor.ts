import type { PatchedFixture } from "../../api/types";

export type PickerColor = { hue: number; saturation: number; brightness: number };
export type ColorProgrammerAssignment = {
  fixtureId: string;
  attribute: string;
  value: number;
};

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

export function interpolatePickerRange(
  count: number,
  start: PickerColor,
  end: PickerColor,
): PickerColor[] {
  if (count <= 0) return [];
  if (count === 1) return [end];
  return Array.from({ length: count }, (_, index) => {
    if (index === 0) return { ...start, brightness: end.brightness };
    if (index === count - 1) return end;
    const ratio = index / (count - 1);
    return {
      hue: start.hue + (end.hue - start.hue) * ratio,
      saturation:
        start.saturation + (end.saturation - start.saturation) * ratio,
      brightness: end.brightness,
    };
  });
}

/** Resolves RGB or CMY values for each ordered selected fixture/head. */
export function colorProgrammerAssignments(
  selectedFixtures: readonly string[],
  patch: readonly PatchedFixture[],
  colors: PickerColor[],
): ColorProgrammerAssignment[] {
  return selectedFixtures.flatMap((fixtureId, index) => {
    const fixture = patch.find(
      (candidate) =>
        candidate.fixture_id === fixtureId ||
        candidate.logical_heads.some((head) => head.fixture_id === fixtureId),
    );
    if (!fixture) return [];
    const logicalHead = fixture.logical_heads.find(
      (head) => head.fixture_id === fixtureId,
    );
    const heads = logicalHead
      ? fixture.definition.heads.filter(
          (head) => head.index === logicalHead.head_index,
        )
      : fixture.definition.heads.filter((head) => head.shared);
    const attributes = new Set(
      heads.flatMap((head) =>
        head.parameters.map((parameter) => parameter.attribute),
      ),
    );
    const color = colors[index];
    if (!color) return [];
    const [red, green, blue] = hsvToRgb(color);
    const values: Array<[string, number]> = [
      ["color.red", red],
      ["color.green", green],
      ["color.blue", blue],
      ["color.cyan", 1 - red],
      ["color.magenta", 1 - green],
      ["color.yellow", 1 - blue],
    ];
    return values.flatMap(([attribute, value]) =>
      attributes.has(attribute) ? [{ fixtureId, attribute, value }] : [],
    );
  });
}
