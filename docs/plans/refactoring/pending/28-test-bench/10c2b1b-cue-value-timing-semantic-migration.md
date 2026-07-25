# 10c2b1b — Cue value timing semantic migration

## Outcome

Migrate the ordinary `CUE-004` UI workflow.

## Scope

- `CUE-004` in `tests/cueSemanticContracts/timingScenarios.ts`

## Done gate

- Per-value fade/delay overrides retain exact boundaries against Cue fallback.
- Force Cue Timing remains reversible without rewriting stored value timing.
- The paired API half remains unchanged.
- Focused API/UI cases, architecture, inventory, and parallel stress pass.
