# 10c2a — Cue recording and reconstruction semantic migration

## Outcome

Migrate the three ordinary Cue recording and cue-only reconstruction UI cases.

## Scope

- `CUE-008` and `CUE-001` in
  `tests/cueSemanticContracts/recordingScenarios.ts`
- `CUE-002` in `tests/cueSemanticContracts/cueOnlyScenarios.ts`

## Done gate

- Blind Preload recording, decimal insertion, Record operations, sequential GO,
  and direct-jump reconstruction retain their exact Cue and output contracts.
- Paired API halves and supplemental boundaries remain unchanged.
- Focused API/UI cases, architecture, inventory, and parallel stress pass.
