# 10c2e5 — Preload capture mask semantic migration

## Outcome

Migrate the ordinary `PRELOAD-005` UI case.

## Scope

- `tests/preloadVirtualPlaybackContracts/captureMaskScenarios.ts`

## Done gate

- Every capture mask keeps disabled domains live and enabled domains blind.
- Existing API and supplemental boundaries remain unchanged.
- Focused API/UI cases, architecture, inventory, and stress pass.

## Result

- Added a public Preload Settings handle that uses the visible Setup switches,
  persists each mask, reloads the Programmer section, and verifies both the
  typed configuration and visible control state.
- Replaced the ordinary paired UI half with semantic-world `PRELOAD-005`
  coverage for all eight independent capture-domain combinations.
- Retained the API half plus the disabled-domain and visible Settings
  supplementals unchanged.
- Verified the focused semantic UI case, retained API and both supplemental
  cases, 20-run parallel UI stress, control-ui build, semantic documentation,
  architecture, source-size, inventory, formatting, and diff checks.
