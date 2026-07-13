import type { FixtureDefinition, PatchedFixture, VisualizationSnapshot } from "../api/types";

type FixtureHead = FixtureDefinition["heads"][number];

export interface FixtureSheetTarget {
  fixture: PatchedFixture;
  fixtureId: string;
  displayId: string | number;
  name: string;
  heads: FixtureHead[];
  order: number;
}

export function fixtureSheetTargets(fixture: PatchedFixture): FixtureSheetTarget[] {
  const fixtureName = fixture.name || fixture.definition.name || fixture.definition.model;
  if (!fixture.logical_heads.length) {
    return [{
      fixture,
      fixtureId: fixture.fixture_id,
      displayId: fixture.fixture_number ?? "—",
      name: fixtureName,
      heads: fixture.definition.heads,
      order: 0,
    }];
  }

  const prefix = fixture.fixture_number == null ? "—" : String(fixture.fixture_number);
  const targets: FixtureSheetTarget[] = [{
    fixture,
    fixtureId: fixture.fixture_id,
    displayId: `${prefix}.0`,
    name: `${fixtureName} · Master`,
    heads: fixture.definition.heads.filter((head) => head.shared),
    order: 0,
  }];
  fixture.definition.heads.filter((head) => !head.shared).forEach((head, index) => {
    const patched = fixture.logical_heads.find((candidate) => candidate.head_index === head.index);
    if (!patched) return;
    targets.push({
      fixture,
      fixtureId: patched.fixture_id,
      displayId: `${prefix}.${index + 1}`,
      name: `${fixtureName} · ${head.name}`,
      heads: [head],
      order: index + 1,
    });
  });
  return targets;
}

export function targetHasAttribute(target: FixtureSheetTarget, attribute: string) {
  return target.heads.some((head) =>
    head.parameters.some((parameter) => parameter.attribute === attribute),
  );
}

export function targetDefault(
  target: FixtureSheetTarget,
  attribute: string,
  fallback = 0,
) {
  return target.heads
    .flatMap((head) => head.parameters)
    .find((parameter) => parameter.attribute === attribute)?.default ?? fallback;
}

export function targetValue(
  snapshot: VisualizationSnapshot | null,
  target: FixtureSheetTarget,
  attribute: string,
  fallback = 0,
) {
  if (!targetHasAttribute(target, attribute)) return fallback;
  const live = snapshot?.values.find(
    (entry) =>
      entry.fixture_id === target.fixtureId &&
      entry.attribute === attribute &&
      entry.value.kind === "normalized",
  )?.value;
  return live?.kind === "normalized"
    ? live.value
    : targetDefault(target, attribute, fallback);
}
