# 10c2b — Cue timing, trigger, and selection semantic migration

## Outcome

Migrate the five ordinary Cue timing, trigger, and implicit-selection UI cases.

## Scope

- `CUE-003` and `CUE-004` in
  `tests/cueSemanticContracts/timingScenarios.ts`
- `CUE-005` in `tests/cueSemanticContracts/triggerScenarios.ts`
- `CUE-006` and `CUE-007` in
  `tests/cueSemanticContracts/selectionScenarios.ts`

## Done gate

- GO/pause/resume/back/release, per-value timing, Force Cue Timing, GO/FOLLOW/TIME,
  implicit Cuelist selection, and explicit-zero tracking retain exact boundaries.
- Paired API halves and supplemental boundaries remain unchanged.
- Focused API/UI cases, architecture, inventory, and parallel stress pass.
