# 10c2e3 — Preload layout persistence semantic migration

## Outcome

Migrate the ordinary `PRELOAD-003` UI case.

## Scope

- `tests/preloadVirtualPlaybackContracts/layoutPersistenceScenarios.ts`

## Done gate

- Virtual Playback grids, source assignments, button layouts, and persistence
  retain exact contracts.
- Existing API and supplemental boundaries remain unchanged.
- Focused API/UI cases, architecture, inventory, and stress pass.

## Result

- Added a public Virtual Playback bench handle for visible source assignment,
  top-button configuration, cell activation, and reload persistence checks.
- Completed the Desktop pane DSL's existing Virtual Playback row/column
  configuration and exposed fader/button-count topology through the typed
  Playback configuration intent.
- Replaced the ordinary paired UI half with semantic-world `PRELOAD-003`
  coverage while retaining its API half; split the independent `VPB-007`
  exclusion-zone migration into the next pending chunk.
- Verified the focused semantic UI case, retained API case, 20-run parallel UI
  stress, control-ui build, semantic documentation, architecture, source-size,
  inventory, formatting, and diff checks.
