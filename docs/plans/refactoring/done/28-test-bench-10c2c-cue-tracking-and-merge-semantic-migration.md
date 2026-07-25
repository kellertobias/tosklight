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

## Result

- Added marked semantic UI scenarios for CUE-010, MERGE-002, and MERGE-003
  while retaining their paired API halves unchanged.
- Rebuilt Cue tracking and independent Sequence arbitration through public
  Record, Encoder, Playback, command-line, and resolved-output intent.
- Added public Cuelist priority configuration, programmer-priority control,
  resolved logical fixture-value assertions, playback auto-Off configuration,
  empty button assignments, and truthful click-to-toggle Temp behavior.
- Covered full and partial overwrite, enabled-state preservation, Flash hold
  restoration, Temp toggle restoration, explicit playback selection, and
  address-local retriggering through visible operator controls.
- Regenerated the 308-case inventory (49 root files, 88 pending rows) and
  34-scenario semantic catalog.
- Passed the control-UI build/type check, focused execution, five-way API/UI
  stress (`30 passed`), semantic documentation, architecture, inventory,
  formatting, and diff checks.
