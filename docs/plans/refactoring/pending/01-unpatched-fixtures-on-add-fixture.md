# Unpatched Fixtures from Add Fixture

## Status and source contract

Pending and explicitly required for implementation. Follow
[`../../Next/51-unpatched-fixtures-on-add-fixture.md`](../../Next/51-unpatched-fixtures-on-add-fixture.md)
as the authoritative operator contract.

Estimated effort: 0.5–1 Codex day.

## Required work

1. Add an explicit **Empty** choice to the Add Fixture placement/address step.
2. Model empty placement as absent optional universe/address data across TypeScript, generated
   wire contracts, Rust boundary validation, Show Patch service, and persistence.
3. Release preview footprint reservations when switching from a concrete address to **Empty** and
   reacquire normal validation when switching back.
4. Make bulk add create every requested fixture unpatched without address auto-advance.
5. Keep fixture identity, number, profile, mode, layer, geometry, selection, programming, Group,
   Preset, Cue, Highlight, Fixture Sheet, and Stage behavior intact; suppress DMX output only.
6. Show the established unpatched label in the confirmation summary and Show Patch row.
7. Preserve existing unpatched fixtures and old show files without migration loss.

## Acceptance and verification

- Cover one and bulk unpatched additions through the real UI and typed API action.
- Save/reload an unpatched fixture through real server startup.
- Program it, store and recall it in a Group, display it in Fixture Sheet and Stage, then patch it
  without changing identity or stored programming.
- Assert no DMX output before patch and normal output afterward.
- Run focused Patch/service, persistence, output, frontend, API/UI Playwright, and desktop checks.

## Non-goals

Do not treat unpatched as deleted, disabled, or absent from the show. Do not auto-assign a fallback
address.
