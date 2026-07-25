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

## Result

- Added a marked `CUE-004` semantic UI scenario that records a two-second
  per-group value fade against a three-second Cue fallback and a nine-second
  Programmer Fade, then proves the rendered DMX boundary at 1,999 ms and
  2,000 ms.
- Added a public Cue expectation for group-value fade/delay timing so root
  scenarios do not inspect raw Cuelist storage.
- Retained the paired API half unchanged for the distinct per-fixture delay,
  Force Cue Timing reversibility, and stored-timing byte-preservation
  contract; only its migrated UI registration was removed.
- Regenerated the 308-case migration inventory and 28-scenario semantic test
  catalog.
- Passed bench type checking, focused API/UI execution, five-way parallel
  stress (`10 passed`), semantic documentation checks, architecture checks,
  inventory checks, and `git diff --check`.
