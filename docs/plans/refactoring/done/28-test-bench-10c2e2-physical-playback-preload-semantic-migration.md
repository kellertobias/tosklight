# 10c2e2 — Physical Playback Preload semantic migration

## Outcome

Migrate the ordinary `PRELOAD-002` UI case.

## Scope

- `tests/preloadVirtualPlaybackContracts/physicalPlaybackPreloadScenarios.ts`

## Done gate

- Physical Playback Preload retains ordered verbs, timing, release scope, and
  live Flash/fader exclusions.
- Existing API, OSC, and supplemental boundaries remain unchanged.
- Focused API/UI/OSC cases, architecture, inventory, and stress pass.

## Result

- Added declarative Preload capture-mask configuration and a semantic pending
  Playback-action-order oracle. Preload active state now correctly represents
  every blind capture mode, not only programmer capture.
- Migrated `PRELOAD-002` to
  `tests/54-semantic-physical-playback-preload.spec.ts`, covering the visible
  Toggle, GO, GO-minus, Off, On, Temp-on, and Temp-off order, representative
  committed runtime outcomes, and non-destructive release.
- Removed the intentionally skipped ordinary paired UI registration while
  retaining its API case and both supplemental API/UI contracts. The exact
  per-verb outcomes, shared activation timestamp, live Flash/fader exclusions,
  and timing remain in those unchanged contracts.
- Inventory: 309 root cases, 80 pending semantic migrations, and 42 semantic
  world scenarios.
- Verification passed: focused semantic UI (1 pass); PRELOAD-002 API, semantic
  UI, and supplemental stress (20 passes over five repetitions); control-ui
  build; Biome; source-size ratchet; architecture; inventory write/check;
  semantic documentation write/check and its 8 tests.
