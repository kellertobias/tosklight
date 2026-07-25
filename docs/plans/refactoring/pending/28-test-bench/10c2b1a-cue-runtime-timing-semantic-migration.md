# 10c2b1a — Cue runtime timing semantic migration

## Outcome

Migrate the ordinary `CUE-003` UI workflow.

## Scope

- `CUE-003` in `tests/cueSemanticContracts/timingScenarios.ts`

## Done gate

- GO, pause, resume, back, and release retain their exact application-time and
  output boundaries through public Playback helpers.
- The paired API half remains unchanged.
- Focused API/UI cases, architecture, inventory, and parallel stress pass.
