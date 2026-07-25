# 10c2e1 — Programmer Preload semantic migration

## Outcome

Migrate the ordinary Programmer Preload UI case.

## Scope

- `PRELOAD-001` in
  `tests/preloadVirtualPlaybackContracts/programmerPreloadScenarios.ts`

## Done gate

- Programmer Preload retains blind programming, timed commit, clear, and
  release behavior.
- Existing API, OSC, supplemental, and intentionally deferred boundaries remain
  unchanged.
- Focused API/UI/OSC cases, architecture, inventory, and parallel stress pass.

## Result

- Split physical, layout/Virtual Playback, virtual, capture-mask, and combined
  Preload work into ordered chunks 10c2e2 through 10c2e6.
- Migrated the ordinary `PRELOAD-001` case to
  `tests/53-semantic-programmer-preload.spec.ts`. It arms Preload visibly and
  exercises the typed fixture-value projection through its matching API
  commit/release route; the unchanged API and supplemental cases retain exact
  group timing, physical GO, and command-line feedback contracts.
- Corrected the public Preload helper so API commit exits capture mode, UI
  `PRELOAD GO` keeps it armed, default release preserves the established API
  behavior, and an explicit UI hold-release route is available.
- Inventory: 309 root cases, 81 pending semantic migrations, and 41 semantic
  world scenarios.
- Verification passed: focused semantic case (1 pass); PRELOAD-001 API,
  semantic UI, and supplemental stress (20 passes over five repetitions);
  control-ui build; Biome; source-size ratchet; architecture; inventory
  write/check; semantic documentation write/check and its 8 tests.
