import type { PatchedFixture, VisualizationSnapshot } from "../api/types";

export function fixtureTargetIds(fixture: PatchedFixture) {
  return [fixture.fixture_id, ...fixture.logical_heads.map((head) => head.fixture_id)];
}

export function fixtureDefault(
  fixture: PatchedFixture,
  attribute: string,
  fallback = 0,
) {
  return (
    fixture.definition.heads
      .flatMap((head) => head.parameters)
      .find((parameter) => parameter.attribute === attribute)?.default ?? fallback
  );
}

export function fixtureValue(
  snapshot: VisualizationSnapshot | null,
  fixture: PatchedFixture,
  attribute: string,
  fallback = 0,
) {
  const ids = fixtureTargetIds(fixture);
  const live = snapshot?.values.find(
    (entry) =>
      ids.includes(entry.fixture_id) &&
      entry.attribute === attribute &&
      entry.value.kind === "normalized",
  )?.value;
  return live?.kind === "normalized"
    ? live.value
    : fixtureDefault(fixture, attribute, fallback);
}
