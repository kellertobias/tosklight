# 10c2d — Cuelist edit, settings, and navigation semantic migration

## Outcome

Migrate the six ordinary Cue move/copy, Speed Group, Cuelist edit/settings, and
Cue Go To/Load UI cases.

## Scope

- `CUE-009` and `CMD-002` in
  `tests/02-cues-tracking-and-arbitration.spec.ts`
- `CUE-011`, `CUE-012`, and `CUE-013` in
  `tests/06-cuelist-view-and-settings.spec.ts`
- `CUE-014` in `tests/09-cue-go-to-load.spec.ts`

## Done gate

- Move/Copy axes, five Speed Groups, Cue identity and renumbering, deletion,
  arbitration/wrapping/restart/Chaser settings, and desk-local Go To/Load
  selection retain exact contracts.
- Existing API, restart, wire, and supplemental boundaries remain unchanged.
- Focused API/UI/wire/restart cases, architecture, inventory, and stress pass.
