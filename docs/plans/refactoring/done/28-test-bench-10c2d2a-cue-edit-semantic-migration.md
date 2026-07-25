# 10c2d2a — Cue edit semantic migration

## Outcome

Migrate the ordinary Cuelist View cue-edit UI case split from 10c2d.

## Scope

- `CUE-011` in `tests/06-cuelist-view-and-settings.spec.ts`

## Done gate

- Cue identity, editing, renumbering, and persistence retain exact contracts.
- Existing API, restart, wire, and supplemental boundaries remain unchanged.
- Focused API/UI/restart cases, architecture, inventory, and stress pass.

## Result

- Split the remaining independent Cuelist Settings and active-Cue deletion work
  into ordered chunks 10c2d2b and 10c2d2c.
- Added a semantic Cuelist View handle for opening an assigned Cuelist, selecting
  Cues, editing metadata and triggers, rejecting invalid values, inspecting
  settings, and reopening persisted state. Every accepted field mutation waits
  for the authoritative Cuelist revision.
- Migrated the ordinary `CUE-011` UI case to
  `tests/50-semantic-cue-editing.spec.ts`; its API and supplemental renumber
  contracts remain registered in the original spec.
- Inventory: 308 root cases, 84 pending semantic migrations, and 38 semantic
  world scenarios.
- Verification passed: focused semantic UI (1 pass); CUE-011 API, semantic UI,
  and supplemental stress (15 passes over five repetitions); control-ui build;
  Biome; source-size ratchet; architecture; inventory write/check; semantic
  documentation write/check and its 8 tests; and `git diff --check`.
