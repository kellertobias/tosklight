# 10c2e — Preload and Virtual Playback semantic migration

## Outcome

Migrate all seven ordinary Preload and Virtual Playback UI cases.

## Scope

- `PRELOAD-001` through `PRELOAD-006` and `VPB-007` in
  `tests/06-preload-modes-and-virtual-playbacks.spec.ts` and its scenario
  support modules.

## Done gate

- Programmer, physical, combined, and virtual Preload modes retain blind/timed
  behavior, ordered verbs, capture masks, atomic commit, release scope, grids,
  and exclusion-zone authority.
- Existing API, OSC, supplemental, and intentionally deferred boundaries remain
  unchanged.
- Focused API/UI/OSC cases, architecture, inventory, and parallel stress pass.
