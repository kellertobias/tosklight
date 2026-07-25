# 10c2d2b — Cuelist settings semantic migration

## Outcome

Migrate the ordinary Cuelist Settings UI case split from 10c2d2.

## Scope

- `CUE-012` in `tests/06-cuelist-view-and-settings.spec.ts`

## Done gate

- Arbitration, wrapping, restart, timing, and Chaser settings retain exact contracts.
- Existing API, restart, wire, and supplemental boundaries remain unchanged.
- Focused API/UI/restart cases, architecture, inventory, and stress pass.

## Result

- Extended the semantic Cuelist View seam with a typed settings handle for
  Sequence/Chaser mode, priority, intensity arbitration, wrapping, restart,
  timing overrides, speed multiplier, and Chaser crossfade.
- Migrated the ordinary `CUE-012` UI contract to
  `tests/51-semantic-cuelist-settings.spec.ts`; retained the original engine/API
  case and registered its legacy-data/restart path explicitly as `@restart`.
- Inventory: 309 root cases, 83 pending semantic migrations, and 39 semantic
  world scenarios.
- Verification passed: focused semantic UI (1 pass); CUE-012 API, restart, and
  semantic UI stress (15 passes over five repetitions); control-ui build;
  Biome; source-size ratchet; architecture; inventory write/check; semantic
  documentation write/check and its 8 tests.
