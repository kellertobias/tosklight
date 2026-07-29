import type { ApiDriver } from "../bench/core/api";
import { plannedDemoFamilyNumbers, plannedDemoRoleNumbers } from "./plannedDemoManifest";

interface PatchedTargetFixture {
  fixture_id: string;
  fixture_number: number | null;
  logical_heads?: Array<{ fixture_id: string }>;
}

const COLORS = [
  ["Red", 1, 0, 0], ["Orange", 1, 0.35, 0], ["Yellow", 1, 1, 0],
  ["Lime", 0.55, 1, 0], ["Green", 0, 1, 0], ["Teal", 0, 0.55, 0.45],
  ["Cyan", 0, 1, 1], ["Light Blue", 0.25, 0.65, 1], ["Dark Blue", 0, 0, 1],
  ["Purple", 0.45, 0, 0.8], ["Magenta", 1, 0, 1], ["White", 1, 1, 1],
  ["Tungsten White", 1, 0.62, 0.32],
] as const;

const POSITIONS = [
  ["Down", 0.5, 0.25], ["Up", 0.5, 0.82], ["Center", 0.5, 0.5],
  ["Fan Out", 0.15, 0.58], ["Blind", 0.5, 0.05],
  ["Cross 1", 0.25, 0.62], ["Cross 2", 0.75, 0.62],
] as const;

const BEAM = [
  ["Open", "gobo.rotating_gobo_wheel", 0],
  ["Dot", "gobo.rotating_gobo_wheel", 0.2],
  ["Circle", "gobo.rotating_gobo_wheel", 0.4],
  ["Line", "gobo.rotating_gobo_wheel", 0.6],
  ["Jungle", "gobo.rotating_gobo_wheel", 0.8],
  ["Gobo Rotation", "gobo.gobo_index_rotation", 0.75],
  ["No Gobo Rotation", "gobo.gobo_index_rotation", 0.5],
  ["Prism", "prism.prism", 1],
  ["No Prism", "prism.prism", 0],
  ["Prism Rotation", "prism.prism_rotation", 0.75],
] as const;

export async function installPlannedDemoPresets(
  api: ApiDriver,
  showId: string,
  fixtures: readonly PatchedTargetFixture[],
) {
  const byNumber = new Map(fixtures.flatMap((fixture) =>
    fixture.fixture_number == null ? [] : [[fixture.fixture_number, targetIds(fixture)] as const]));
  const colorTargets = targets(byNumber, [
    ...plannedDemoFamilyNumbers("profile"),
    ...plannedDemoFamilyNumbers("wash"),
    ...plannedDemoFamilyNumbers("led"),
    ...plannedDemoRoleNumbers("Sunstrips"),
  ]);
  const movingTargets = targets(byNumber, [
    ...plannedDemoFamilyNumbers("profile"),
    ...plannedDemoFamilyNumbers("wash"),
  ]);
  const profileTargets = targets(byNumber, plannedDemoFamilyNumbers("profile"));
  for (const [index, [name, red, green, blue]] of COLORS.entries())
    await put(api, showId, "preset", `2.${index + 1}`, preset(
      index + 1, name, "Color", colorTargets,
      { "color.red": red, "color.green": green, "color.blue": blue },
    ));
  for (const [index, [name, pan, tilt]] of POSITIONS.entries())
    await put(api, showId, "preset", `3.${index + 1}`, preset(
      index + 1, name, "Position", movingTargets, { pan, tilt },
    ));
  for (const [index, [name, attribute, value]] of BEAM.entries())
    await put(api, showId, "preset", `4.${index + 1}`, preset(
      index + 1, name, "Beam", profileTargets, { [attribute]: value },
    ));
  return { colors: COLORS.length, positions: POSITIONS.length, beam: BEAM.length };
}

function targets(byNumber: Map<number, string[]>, numbers: readonly number[]) {
  return numbers.flatMap((number) => {
    const found = byNumber.get(number);
    if (!found) throw new Error(`Preset references missing fixture ${number}`);
    return found;
  });
}

function preset(
  number: number,
  name: string,
  family: string,
  fixtureIds: readonly string[],
  attributes: Readonly<Record<string, number>>,
) {
  return {
    name,
    family,
    number,
    values: Object.fromEntries(fixtureIds.map((fixtureId) => [
      fixtureId,
      Object.fromEntries(Object.entries(attributes).map(([attribute, value]) => [
        attribute, { kind: "normalized", value },
      ])),
    ])),
    group_values: {},
  };
}

function targetIds(fixture: PatchedTargetFixture) {
  return fixture.logical_heads?.length
    ? fixture.logical_heads.map((head) => head.fixture_id)
    : [fixture.fixture_id];
}

async function put(api: ApiDriver, showId: string, kind: string, id: string, body: unknown) {
  await api.seedShowObject(showId, kind, id, body, 0);
}
