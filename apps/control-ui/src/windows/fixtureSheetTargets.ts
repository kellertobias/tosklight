import type { FixtureDefinition, PatchedFixture, VisualizationSnapshot } from "../api/types";
import type { FixtureSheetIncludedHeads } from "../types";

type FixtureHead = FixtureDefinition["heads"][number];

export interface FixtureSheetTarget {
  fixture: PatchedFixture;
  fixtureId: string;
  displayId: string | number;
  name: string;
  heads: FixtureHead[];
  order: number;
  indented: boolean;
}

export function fixtureSheetTargets(
  fixture: PatchedFixture,
  includedHeads: FixtureSheetIncludedHeads = "all",
): FixtureSheetTarget[] {
  const fixtureName = fixture.name || fixture.definition.name || fixture.definition.model;
  if (!fixture.logical_heads.length) {
    return [{
      fixture,
      fixtureId: fixture.fixture_id,
      displayId: fixture.virtual_fixture_number != null ? `0.${fixture.virtual_fixture_number}` : fixture.fixture_number ?? "—",
      name: fixtureName,
      heads: fixture.definition.heads,
      order: 0,
      indented: false,
    }];
  }

  const prefix = fixture.virtual_fixture_number != null ? `0.${fixture.virtual_fixture_number}` : fixture.fixture_number == null ? "—" : String(fixture.fixture_number);
  const targets: FixtureSheetTarget[] = [{
    fixture,
    fixtureId: fixture.fixture_id,
    displayId: includedHeads === "no-sub-heads" ? prefix : `${prefix}.0`,
    name: `${fixtureName} · Master`,
    heads: fixture.definition.heads.filter((head) => head.shared),
    order: 0,
    indented: false,
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
      indented: includedHeads !== "no-master-heads",
    });
  });
  if (includedHeads === "no-sub-heads") return targets.slice(0, 1);
  if (includedHeads === "no-master-heads") return targets.slice(1);
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
