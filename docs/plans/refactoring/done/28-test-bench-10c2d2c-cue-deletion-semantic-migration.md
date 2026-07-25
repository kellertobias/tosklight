# 10c2d2c — Cue deletion semantic migration

## Outcome

Migrate the ordinary active-Cue deletion UI case split from 10c2d2.

## Scope

- `CUE-013` in `tests/06-cuelist-view-and-settings.spec.ts`

## Done gate

- Active deletion, output hold, and GO/GO-minus anchoring retain exact contracts.
- Existing API, wire, and supplemental boundaries remain unchanged.
- Focused API/UI cases, architecture, inventory, and stress pass.

## Result

- Migrated the ordinary `CUE-013` UI case to
  `tests/52-semantic-cue-deletion.spec.ts` using only the public recording, Cue,
  Playback, clock, runtime, and fixture-DMX vocabulary.
- The scenario proves that deleting the active Cue removes its stored data,
  retains its output/runtime hold, and anchors GO and GO-minus to the surviving
  next and previous Cues. The original API and supplemental sole-Cue safeguards
  remain unchanged.
- Inventory: 309 root cases, 82 pending semantic migrations, and 40 semantic
  world scenarios.
- Verification passed: focused semantic UI (1 pass); CUE-013 API, semantic UI,
  and supplemental stress (15 passes over five repetitions); Biome; source-size
  ratchet; architecture; inventory write/check; semantic documentation
  write/check and its 8 tests.
