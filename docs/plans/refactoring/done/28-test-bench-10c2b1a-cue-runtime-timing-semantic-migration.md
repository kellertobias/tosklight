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

## Result

- Added a semantic browser case for exact GO, pause, resume, GO minus, and
  release behavior across deterministic application-time boundaries.
- Added public Playback pause/resume and Cuelist release semantics, including
  automatic restoration of the visible playback surface.
- Retained the paired API case and removed only its superseded UI registration.
  The inventory remains at 308 cases across 46 root files, with 94 pending rows.

Verification:

- Control UI typecheck, architecture, inventory, formatting, and diff checks:
  passed;
- paired API and five-bench isolation checks: 2 passed in parallel;
- corrected semantic browser scenario: passed.
