export type StageSelectionGesture = {
  fixtureId: string;
  orderedFixtureIds: string[];
  selectedFixtureIds: string[];
  anchorFixtureId: string | null;
  additive: boolean;
  range: boolean;
};

/** Applies stage selection while retaining deterministic stage order. */
export function applyStageSelection({ fixtureId, orderedFixtureIds, selectedFixtureIds, anchorFixtureId, additive, range }: StageSelectionGesture): string[] {
  if (!fixtureId) return selectedFixtureIds;
  if (range && anchorFixtureId) {
    const anchor = orderedFixtureIds.indexOf(anchorFixtureId);
    const target = orderedFixtureIds.indexOf(fixtureId);
    if (anchor >= 0 && target >= 0) {
      const members = orderedFixtureIds.slice(Math.min(anchor, target), Math.max(anchor, target) + 1);
      return additive ? orderedUnion(selectedFixtureIds, members) : members;
    }
  }
  if (additive) {
    return selectedFixtureIds.includes(fixtureId)
      ? selectedFixtureIds.filter((id) => id !== fixtureId)
      : orderedUnion(selectedFixtureIds, [fixtureId]);
  }
  return [fixtureId];
}

export function applyMarqueeSelection(current: string[], hits: string[], additive: boolean): string[] {
  return additive ? orderedUnion(current, hits) : hits;
}

function orderedUnion(first: string[], second: string[]): string[] {
  const seen = new Set(first);
  return [...first, ...second.filter((id) => !seen.has(id) && Boolean(seen.add(id)))];
}
