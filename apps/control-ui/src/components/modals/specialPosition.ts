import type { PatchedFixture, VisualizationSnapshot } from "../../api/types";

export type LampPosition = { pan: number; tilt: number };

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
