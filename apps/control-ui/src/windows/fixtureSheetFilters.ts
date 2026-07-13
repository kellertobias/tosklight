import type { CueList, PatchedFixture, ProgrammerState, StoredGroup } from "../api/types";

type Group = { id: string; body: StoredGroup };

function fixtureIdsForGroups(groupIds: Iterable<string>, groups: Group[]) {
  const wanted = new Set(groupIds);
  return new Set(
    groups
      .filter((group) => wanted.has(group.id))
      .flatMap((group) => group.body.fixtures),
  );
}

export function activeProgrammerFixtureIds(
  programmer: ProgrammerState | undefined,
  groups: Group[],
) {
  const active = fixtureIdsForGroups(
    Object.keys(programmer?.group_values ?? {}),
    groups,
  );
  for (const value of programmer?.values ?? []) {
    const fixtureId = (value as { fixture_id?: unknown }).fixture_id;
    if (typeof fixtureId === "string") active.add(fixtureId);
  }
  return active;
}

export function cueListFixtureIds(cueList: CueList | undefined, groups: Group[]) {
  if (!cueList) return null;
  const fixtureIds = new Set(
    cueList.cues.flatMap((cue) => cue.changes.map((change) => change.fixture_id)),
  );
  const groupIds = cueList.cues.flatMap((cue) =>
    (cue.group_changes ?? []).map((change) => change.group_id),
  );
  for (const fixtureId of fixtureIdsForGroups(groupIds, groups))
    fixtureIds.add(fixtureId);
  return fixtureIds;
}

export function fixtureIsIncluded(
  fixture: PatchedFixture,
  fixtureIds: Set<string>,
) {
  return [
    fixture.fixture_id,
    ...fixture.logical_heads.map((head) => head.fixture_id),
  ].some((fixtureId) => fixtureIds.has(fixtureId));
}

export function compareFixtureIds(a: PatchedFixture, b: PatchedFixture) {
  const aNumber = a.fixture_number ?? Number.POSITIVE_INFINITY;
  const bNumber = b.fixture_number ?? Number.POSITIVE_INFINITY;
  return aNumber - bNumber || a.fixture_id.localeCompare(b.fixture_id);
}
