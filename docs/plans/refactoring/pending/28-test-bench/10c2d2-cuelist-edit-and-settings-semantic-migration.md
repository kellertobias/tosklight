# 10c2d2 — Cuelist edit and settings semantic migration

## Outcome

Migrate the three ordinary Cuelist edit, settings, renumber, and deletion UI
cases split from 10c2d.

## Scope

- `CUE-011`, `CUE-012`, and `CUE-013` in
  `tests/06-cuelist-view-and-settings.spec.ts`

## Done gate

- Cue identity and renumbering, deletion, arbitration, wrapping, restart,
  timing, and Chaser settings retain exact contracts.
- Existing API, restart, wire, and supplemental boundaries remain unchanged.
- Focused API/UI/restart cases, architecture, inventory, and stress pass.
