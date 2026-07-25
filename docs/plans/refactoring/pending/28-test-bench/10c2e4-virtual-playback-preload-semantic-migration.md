# 10c2e4 — Virtual Playback Preload semantic migration

## Outcome

Migrate the ordinary `PRELOAD-004` UI case.

## Scope

- `tests/preloadVirtualPlaybackContracts/virtualPlaybackPreloadScenarios.ts`

## Done gate

- Virtual Preload retains blind/timed behavior, feedback, release, and disabled
  domain behavior.
- Existing API, OSC, supplemental, and intentionally deferred boundaries remain
  unchanged.
- Focused API/UI/OSC cases, architecture, inventory, and stress pass.
