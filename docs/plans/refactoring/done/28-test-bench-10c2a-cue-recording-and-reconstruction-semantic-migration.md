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

## Result

- Added three semantic browser cases for blind Preload recording, decimal Cue
  insertion with overwrite/merge/subtract tracking, and Cue-only restoration
  across sequential GO and direct jumps.
- Extended the public bench with visible Cue-only settings and append gestures;
  command and playback helpers now restore the operator surface they require.
- Retained all three paired API cases and removed only their superseded UI
  registrations. The inventory remains at 308 cases across 45 root files, with
  95 pending rows.

Verification:

- Control UI typecheck, architecture, inventory, formatting, and diff checks:
  passed;
- paired API cases, semantic browser cases, and five-bench isolation stress:
  7 passed using four workers.
