# 03 — Show operator workflows

## Outcome

Implement readable helpers for the show operations an operator performs after setup: create, load,
save, Save As, revisions, defaults, and restart/recovery.

## Public helpers

- `show.create(name)`;
- `show.load(nameOrFixture)`;
- `show.save()`;
- `show.saveAs(name)`;
- `show.saveRevision(name)`;
- `show.loadRevision(show, revision)`;
- `show.loadCleanDefault()`;
- `show.restart(RestartMode.Graceful | RestartMode.Abrupt)`;
- `show.expect.active(...)`, `.revision(...)`, `.dirty(...)`, and `.recovered(...)`.

Each action declares truthful routes:

- visible application menu/dialog;
- typed API intent when independently supported;
- harness only for process restart and deliberately malformed fixture placement.

`show.load` must not be implemented as `show.use`. The UI route opens the real menu and picker,
shows progress, handles errors, and waits for the active-show projection. The API route uses the
typed production contract, never raw object writes.

## Persistence and compatibility

Read `docs/acceptance-criteria.md` before implementation. Preserve:

- portable show versus desk-level data boundaries;
- old-show migration behavior;
- seeded/default data required by existing installations;
- recovery from malformed or legacy active shows;
- explicit revision-copy metadata;
- actionable progress and errors for long-running operations.

## Helper-contract scenarios

1. Create an empty named show through the visible UI and assert it is active.
2. Save As creates the requested new identity without silently renaming the source.
3. Save and reopen preserve representative portable objects.
4. Save a named revision and load it as the documented separate active copy.
5. Load Clean Default follows the product workflow and does not use fixture setup.
6. Graceful restart retains the isolated data directory and active show.
7. Abrupt restart exercises recovery without sharing state with another bench.
8. Loading a malformed or unsupported show produces visible actionable failure and leaves a
   recoverable application state.
9. UI and API variants reach the same normalized show identity where both are meaningful.

## Done gate

- Setup and operator workflows are visibly distinct in code and reports.
- UI helpers operate real menus, modal title-bar actions, progress, and errors.
- Restart and recovery tests remain isolated to test-owned data.
- No helper exposes a raw persisted show-object mutation.

## Result

- Added opaque show handles and semantic create, load, autosave confirmation, Save As, named
  revision, revision-copy load, clean-default, graceful restart, and abrupt restart helpers.
- Visible routes operate the real Show menu and stacked dialogs and wait for both dialog closure
  and the active-show projection. Independent API routes use the typed production show-library
  contract; there is deliberately no fabricated API route for ordinary manual Save.
- Defined `show.expect.dirty(false)` as autosave convergence. `dirty(true)` fails before claiming
  unsupported state because the product has no dirty-show projection and commits changes
  continuously.
- Added test-owned malformed-active-show setup plus recovery-required/recovered expectations over
  real server startup, readiness, bootstrap, the visible actionable recovery dialog, and
  byte-for-byte preservation of the damaged file.
- Split catalog, identity, operator, and recovery responsibilities so the new helper family adds
  no source file above the repository's 400-line design goal.
- `BENCH-SHOW-004` through `BENCH-SHOW-006` cover visible named creation, representative portable
  Save As/reopen, named revisions and revision copies, explicit typed API parity, clean default,
  both restart modes, and malformed-show recovery.

Verification:

- `npm run test:architecture`: passed with no new file above the 400-line design goal.
- `npm run test:bench-types`: passed.
- focused show-operator contract: 3 passed.
- full `npm run test:e2e`: 296 passed / 9 skipped.
