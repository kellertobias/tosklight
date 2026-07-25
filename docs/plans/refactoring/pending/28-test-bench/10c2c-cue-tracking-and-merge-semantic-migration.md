# 10c2c — Cue tracking and merge semantic migration

## Outcome

Migrate the three ordinary Cue attribute-ownership and merge-arbitration UI cases.

## Scope

- `CUE-010` in `tests/cueSemanticContracts/trackingScenarios.ts`
- `MERGE-002` in `tests/cueSemanticContracts/mergeCoexistenceScenarios.ts`
- `MERGE-003` in `tests/cueSemanticContracts/mergeReplacementScenarios.ts`

## Done gate

- Per-attribute tracking and LTP ownership, independent Sequences, retriggering,
  auto-Off, partial overwrite, Flash, and Temp restore retain exact contracts.
- Paired API halves and supplemental boundaries remain unchanged.
- Focused API/UI cases, architecture, inventory, and parallel stress pass.
