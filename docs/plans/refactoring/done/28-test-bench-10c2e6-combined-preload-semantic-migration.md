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

## Result

- Added a semantic `PRELOAD-006` UI scenario that records independent
  programmer, physical Playback, and Virtual Playback sources, verifies the
  exact pending actions, commits them with one shared timestamp, and confirms
  the asymmetric release contract.
- Added typed, polling bench oracles for pending Preload state and atomic commit
  timestamps. Stress testing exposed projection races, so the affected active,
  inactive, and pending assertions now wait for the authoritative projection
  instead of sampling it once.
- Removed the migrated UI surface from the paired low-level case while keeping
  the API and both supplemental ownership/idempotency/UI boundaries intact.
- Verified the focused semantic case, the retained API and supplemental cases,
  a 20-run semantic stress pass, the control UI build, semantic documentation
  generation and checks, compiler tests, architecture, source-size, inventory,
  and diff hygiene.
