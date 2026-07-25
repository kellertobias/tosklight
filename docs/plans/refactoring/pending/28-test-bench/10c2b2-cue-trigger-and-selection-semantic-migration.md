# 10c2b2 — Cue trigger and selection semantic migration

## Outcome

Migrate the three ordinary Cue trigger and implicit-selection UI cases.

## Scope

- `CUE-005` in `tests/cueSemanticContracts/triggerScenarios.ts`
- `CUE-006` and `CUE-007` in
  `tests/cueSemanticContracts/selectionScenarios.ts`

## Done gate

- GO, FOLLOW, and TIME measure from the prior Cue's latest value endpoint.
- Explicit playback selection supplies the implicit Cuelist without following
  execution order, while explicit zeroes block later inserted values.
- Paired API halves and supplemental boundaries remain unchanged.
- Focused API/UI cases, architecture, inventory, and parallel stress pass.
