# 10c2e6 — Combined Preload semantic migration

## Outcome

Migrate the ordinary `PRELOAD-006` UI case.

## Scope

- `tests/preloadVirtualPlaybackContracts/combinedPreloadScenarios.ts`

## Done gate

- Combined Preload retains atomic commit, source ownership, pending state, and
  asymmetric release behavior.
- Existing API and supplemental boundaries remain unchanged.
- Focused API/UI cases, architecture, inventory, and stress pass.
