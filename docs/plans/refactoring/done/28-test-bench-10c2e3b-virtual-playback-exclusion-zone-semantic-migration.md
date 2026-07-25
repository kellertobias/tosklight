# 10c2e3b — Virtual Playback exclusion-zone semantic migration

## Outcome

Migrate the ordinary `VPB-007` UI case.

## Scope

- `tests/preloadVirtualPlaybackContracts/virtualZonePairScenario.ts`

## Done gate

- Virtual Playback exclusion-zone creation stays inert and preserves ordered
  mutual-exclusion authority.
- Existing API and supplemental boundaries remain unchanged.
- Focused API/UI cases, architecture, inventory, and stress pass.

## Result

- Extended the public Virtual Playback bench handle with visible Shift-based
  exclusion-zone creation and a typed persisted-zone oracle.
- Replaced the ordinary paired UI half with semantic-world `VPB-007` coverage
  for inert creation, ordered mutual exclusion, and resolved fixture output;
  retained the API half and the independent Settings/reload supplemental.
- Verified the focused semantic UI case, retained API case, 20-run parallel UI
  stress, control-ui build, semantic documentation, architecture, source-size,
  inventory, formatting, and diff checks.
