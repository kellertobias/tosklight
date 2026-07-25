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
