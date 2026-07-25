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
