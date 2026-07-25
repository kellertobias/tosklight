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

## Result

- Added marked semantic UI scenarios for CUE-005 through CUE-007 while
  retaining their paired API halves unchanged.
- CUE-005 now proves the physical double-TIME entry visibly normalizes to
  FOLLOW/DELAY triggers; the paired API case remains authoritative for the
  1,999/2,000 ms, 5,999/6,000 ms, week-long, and latest-value endpoint
  boundaries.
- Fixed the public Playback selection helper to use the documented Shift+Z
  shortcut before clicking the visible playback representation, then proved
  another Playback can run without changing the explicitly selected Cuelist.
- Rebuilt the explicit-zero scenario through public Group, Record, Playback,
  and DMX intent, including an inserted Cue 3.5 and exact output at all six Cue
  states.
- Added public Cue trigger/group-value expectations and semantic narration for
  the newly exercised command, Cue, and Playback helper paths.
- Regenerated the 308-case inventory (48 root files, 90 pending rows) and
  31-scenario semantic catalog.
- Passed control-UI build/type checking, focused API/UI execution, five-way
  parallel stress (`30 passed`), semantic documentation, architecture,
  inventory, formatting, and diff checks.
