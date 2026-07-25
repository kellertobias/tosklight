# 10c2b1 — Cue timing semantic migration

## Outcome

Migrate the two ordinary Cue application-time and per-value timing UI cases.

## Scope

- `CUE-003` and `CUE-004` in
  `tests/cueSemanticContracts/timingScenarios.ts`

## Done gate

- GO, pause, resume, back, and release retain exact application-time boundaries.
- Per-value timing overrides Cue fallback and Force Cue Timing remains
  reversible.
- Paired API halves and supplemental boundaries remain unchanged.
- Focused API/UI cases, architecture, inventory, and parallel stress pass.
