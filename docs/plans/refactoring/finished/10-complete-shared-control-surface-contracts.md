# Complete Shared Control-surface Contracts

## Goal

Extend the public control-surface contracts in `apps/ui-library` beyond the numeric keypad so
desktop, Hardware Controls, tests, and future applications share stable operator intent and
physical-layout contracts without importing another application's internals.

Estimated effort: 0.3–0.6 Codex day.

## Queue dependency

Pending, blocked until plans 07 and 08 establish the final typed command, event, Highlight, and
interaction names that this cross-application package must expose. Starting earlier would create a
second temporary compatibility contract.

## Required work

1. Inventory stable shared keypad IDs, OSC action/path mappings, Highlight actions, playback
   addressing, encoder actions, and attached physical-layout metadata.
2. Move only proven cross-application contracts into public `@tosklight/ui` exports.
3. Keep transport connections, runtime state, application controllers, and rendering in their
   owning applications.
4. Replace desktop tests that deep-import Hardware Controls internals.
5. Add architecture rules preventing application-to-application source imports.

## Acceptance and verification

- Both applications and root tests consume `@tosklight/ui` exports.
- Current software/hardware action names, layout order, page semantics, and OSC vocabulary remain
  exact.
- Package unit/type tests, both application builds/tests, architecture checks, and hardware
  interaction acceptance pass.

## Result

Completed on 2026-07-26.

### Changes

- Added the focused public `@tosklight/ui/control-surface-contracts` entry point for canonical
  Highlight, Programmer, encoder, navigation, Speed Group, current-page playback, explicit-page
  playback, and fade OSC paths.
- Moved the proven attached-control layout contract into the shared package: Highlight key order
  and coordinates, RECORD/PRELOAD placement, keypad offset, playback/button ranges, encoder slots,
  NAV encoder, and Speed Group numbers.
- Migrated Hardware Controls and root cross-surface helpers to those shared contracts while
  leaving OSC connections, feedback state, application controllers, and rendering in the
  Hardware Controls application.
- Moved the Hardware Controls application interaction test out of the desktop package and removed
  the desktop test's application-to-application imports. Removed the superseded Hardware Controls
  path/layout modules.
- Tightened the shared Programmer keypad type to one digit and canonicalized CLR, UND, REC, and
  PRE to the server vocabulary `clear`, `undo`, `record`, and `preload`.
- Added an architecture boundary covering static imports, re-exports, dynamic imports, `require`,
  and package-manifest dependencies between desktop applications.

### Tests

- `npm run test:ui-package` — 22 files and 147 tests passed.
- `npm run test --workspace @tosklight/light-hardware-controls` — 5 files and 17 tests passed.
- Focused desktop attached-layout and page-summary tests — 2 files and 10 tests passed.
- UI, Hardware Controls, and desktop typechecks passed.
- Hardware Controls and desktop production builds passed.
- `npm run test:architecture` — passed, including the application-to-application boundary.
- Focused live-bench E2E acceptance passed after rerunning outside the sandbox so Playwright could
  bind loopback: HIGHLIGHT-006 and ENCODER-DISPLAY-001 passed against the production hardware
  simulator; OSC-005 and OSC-006 passed for two-desk isolation and current-page playback.

### Limitations

- The complete desktop Vitest suite still has the same two unrelated concurrent expectation
  failures: the Virtual Playbacks test expects the former `unavailable` accessible label, and the
  Groups test expects the former `Built-in ★` option label. All Plan 10 focused tests pass.
- Explicit-page playback remains a transport distinction rather than a Hardware Controls feature:
  the shared contract and unit test retain `/playback/{page}/{slot}` separately from the
  desk-relative `/page-playback/{slot}` path.

### Commit

- `feat(controls): share control-surface contracts`
