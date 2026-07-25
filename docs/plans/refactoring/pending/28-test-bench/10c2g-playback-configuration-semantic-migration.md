# 10c2g — Playback Configuration semantic migration

## Outcome

Migrate all seven ordinary Playback Configuration UI cases.

## Scope

- `PBK-001` through `PBK-006`, including both `PBK-006` cases, in
  `tests/07-playback-configuration.spec.ts` and its scenario support modules.

## Done gate

- Set inspection, assignment and clearing, colors, remapped actions, X-fade,
  Temp/Swap lifetimes, Speed/Group/Grand/Fade masters, and overlapping Group
  Master arbitration retain exact operator contracts.
- Existing API, OSC, wire, and supplemental boundaries remain unchanged.
- Focused API/UI/OSC/wire cases, architecture, inventory, and parallel stress
  pass.
