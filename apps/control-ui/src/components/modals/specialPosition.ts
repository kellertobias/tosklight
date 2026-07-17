import type { PatchedFixture, VisualizationSnapshot } from "../../api/types";

export type LampPosition = { pan: number; tilt: number };
export type ProgrammerHomeAssignment = {
  fixtureId: string;
  attribute: "pan" | "tilt";
  value: number;
};

/** Builds the ordered, per-head Position home gesture for the current selection. */
export function returnHomeAssignments(
  selectedFixtures: string[],
  patch: PatchedFixture[],
): ProgrammerHomeAssignment[] {
  const assignments: ProgrammerHomeAssignment[] = [];
  const seen = new Set<string>();
  for (const fixtureId of selectedFixtures) {
    const fixture = patch.find(
      (candidate) =>
        candidate.fixture_id === fixtureId ||
        candidate.logical_heads.some((head) => head.fixture_id === fixtureId),
    );
    if (!fixture) continue;
    const logicalHead = fixture.logical_heads.find(
      (head) => head.fixture_id === fixtureId,
    );
    const heads = logicalHead
      ? fixture.definition.heads.filter(
          (head) => head.index === logicalHead.head_index,
        )
      : fixture.definition.heads.filter((head) => head.shared);
    for (const attribute of ["pan", "tilt"] as const) {
      const parameter = heads
        .flatMap((head) => head.parameters)
        .find((candidate) => candidate.attribute === attribute);
      if (!parameter) continue;
      const key = `${fixtureId}\u0000${attribute}`;
      if (seen.has(key)) continue;
      seen.add(key);
      const configuredDefault = (parameter as { default?: number }).default;
      assignments.push({
        fixtureId,
        attribute,
        value: Number.isFinite(configuredDefault) ? configuredDefault! : 0.5,
      });
    }
  }
  return assignments;
}

function parameterDefault(
  patch: PatchedFixture[],
  fixtureId: string,
  attribute: "pan" | "tilt",
) {
  for (const fixture of patch) {
    const logicalHead = fixture.logical_heads.find(
      (head) => head.fixture_id === fixtureId,
    );
    if (fixture.fixture_id !== fixtureId && !logicalHead) continue;

    const heads = logicalHead
      ? fixture.definition.heads.filter(
          (head) => head.index === logicalHead.head_index,
        )
      : fixture.definition.heads.filter((head) => head.shared);
    return (
      heads
        .flatMap((head) => head.parameters)
        .find((parameter) => parameter.attribute === attribute)?.default ?? 0
    );
  }
  return 0;
}

/** Resolves a separate physical starting position for every selected lamp. */
export function resolveLampPositions(
  selectedFixtures: string[],
  patch: PatchedFixture[],
  snapshot: VisualizationSnapshot,
) {
  const positions = new Map<string, LampPosition>();
  for (const fixtureId of selectedFixtures) {
    positions.set(fixtureId, {
      pan: parameterDefault(patch, fixtureId, "pan"),
      tilt: parameterDefault(patch, fixtureId, "tilt"),
    });
  }
  for (const entry of snapshot.values) {
    const position = positions.get(entry.fixture_id);
    if (!position || entry.value.kind !== "normalized") continue;
    if (entry.attribute === "pan") position.pan = entry.value.value;
    if (entry.attribute === "tilt") position.tilt = entry.value.value;
  }
  return positions;
}

export function moveLampPositions(
  positions: Map<string, LampPosition>,
  x: number,
  y: number,
  speed: number,
) {
  for (const position of positions.values()) {
    position.pan = Math.max(0, Math.min(1, position.pan + x * speed));
    position.tilt = Math.max(0, Math.min(1, position.tilt + y * speed));
  }
  return positions;
}
