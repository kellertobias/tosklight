# 10c2d2b — Cuelist settings semantic migration

## Outcome

Migrate the ordinary Cuelist Settings UI case split from 10c2d2.

## Scope

- `CUE-012` in `tests/06-cuelist-view-and-settings.spec.ts`

## Done gate

- Arbitration, wrapping, restart, timing, and Chaser settings retain exact contracts.
- Existing API, restart, wire, and supplemental boundaries remain unchanged.
- Focused API/UI/restart cases, architecture, inventory, and stress pass.
