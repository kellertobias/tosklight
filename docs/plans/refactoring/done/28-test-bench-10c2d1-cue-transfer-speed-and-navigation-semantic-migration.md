# 10c2d1 — Cue transfer, Speed Group, and navigation semantic migration

## Outcome

Migrate the three ordinary Cue move/copy, Speed Group, and Cue Go To/Load UI
cases.

## Scope

- `CUE-009` and `CMD-002` in
  `tests/02-cues-tracking-and-arbitration.spec.ts`
- `CUE-014` in `tests/09-cue-go-to-load.spec.ts`

## Done gate

- Move/Copy axes, five Speed Groups, and desk-local Go To/Load selection retain
  exact contracts.
- Existing API, restart, wire, and supplemental boundaries remain unchanged.
- Focused API/UI/wire cases, architecture, inventory, and stress pass.

## Result

- Split the Cuelist-window half into pending chunk 10c2d2 so this independently
  testable command/playback slice could land without mixing window-editor
  infrastructure into it.
- Added marked semantic UI scenarios for CUE-009, CMD-002, and CUE-014 while
  retaining their API, OSC, and supplemental UI boundaries unchanged.
- Added a public Cue transfer-choice dialog contract and taught the visible
  command helper to enter `SPD GRP` through the physical Shift+Time chord.
- Rebuilt twin Cuelists through public Record/Encoder intent and proved
  desk-local selection, Go To, Load, GO, GO minus, and Off behavior through
  visible Playback and command-line controls.
- Exercised all five Speed Groups, decimal BPM, relative adjustment,
  synchronization, visible tap tempo, and manual unlinking.
- Regenerated the 308-case inventory (50 root files, 85 pending rows) and
  37-scenario semantic catalog.
- Passed focused execution, the command-helper unit suite, control-UI
  build/type checking, five-way API/UI/OSC and supplemental stress
  (`55 passed`), semantic documentation, architecture, inventory, formatting,
  and diff checks.
