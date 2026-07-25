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

## Result

- Moved shared Playback Configuration API, UI, model, and observation support
  into the organized
  `apps/control-ui/e2e/bench/playbacks/playback-configuration/` area, with
  separate setup and operator-scenario modules.
- Migrated all seven ordinary UI contracts to semantic scenarios covering
  mutation-free SET inspection, atomic assignment/clear, color and button
  remapping, manual X-fade timing, Temp/Swap lifetimes, all specialized
  masters, and overlapping Group Master HTP arbitration.
- Kept all 21 API, OSC, wire, and supplemental cases in the original suite;
  the retained suite passes unchanged after its support imports were redirected
  to the shared bench area.
- Verified seven focused semantic cases, 21 retained cases, a 35-case parallel
  stress matrix, the control UI build, semantic documentation and compiler
  checks, architecture, source-size, inventory, formatting, and diff hygiene.
