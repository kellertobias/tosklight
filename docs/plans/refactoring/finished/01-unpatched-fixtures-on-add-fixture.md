# Unpatched Fixtures from Add Fixture

## Status and source contract

Finished. Implemented the complete behavior contract in
[`../../Next/51-unpatched-fixtures-on-add-fixture.md`](../../Next/51-unpatched-fixtures-on-add-fixture.md).
That specification is authoritative; this queue file records the execution boundary.

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

## Result

Implemented in the semantic-release commit
`feat(patch): add fixtures without dmx addresses`.

### Changes

- Added an explicit **Address**/**Empty** choice to the Add Fixture placement step. Empty placement
  removes the Universe preview and presents an unpatched confirmation summary; returning to
  Address restores the retained draft and collision validation.
- Added a dedicated bulk path that allocates all requested fixture numbers and stable identities
  while writing nullable assignments for every mode split and never advancing a DMX address.
- Preserved the established **Unpatched** Show Patch label and every existing fixture, selection,
  programming, Group, Stage, Fixture Sheet, Highlight, Cue, and output behavior.
- Confirmed and regression-tested the existing typed TypeScript/Rust Patch contract: matching null
  universe/address pairs are valid, partial pairs fail before side effects, and old unpatched
  portable records remain supported without a schema migration.
- Updated operator help, testing Markdown, and the generated test-bench inventory.

### Tests

- The focused FixturePatchSetup suite passed 25 tests covering single and bulk Empty additions,
  retained concrete drafts, released/reacquired previews, stable profile data, and existing Patch
  interactions.
- The complete Fixture Address Playwright file passed three real UI scenarios. The new
  PATCH-PLACEMENT-002 scenario bulk-added three Empty fixtures, verified their Show Patch rows and
  nullable typed API projections, restarted the isolated server, reopened the show, and repatched
  one fixture without changing its identity or number.
- Focused application, wire, and engine tests passed for bulk nullable placement, malformed partial
  pairs, legacy compiler compatibility, Group/programmer retention, DMX suppression, and output
  after repatching.
- `npm run test:unit` passed the complete Rust workspace, architecture and generated-contract
  gates, 161 bench tests, 2,015 desktop tests, and 16 hardware-control tests.
- `npm run manual` produced and verified the 141-page PDF and offline HTML manual.
- `npm run open` built and launched both Tauri apps. The desktop-owned service reported ready with
  the active show loaded and recovery mode off.

### Limitations

- **Empty** clears the complete new fixture. Operators still use the existing independent split
  fields when only selected splits should remain unpatched.
- This plan reused the established nullable Patch schema and portable-show migration path; no new
  persisted schema version or migration was necessary.
