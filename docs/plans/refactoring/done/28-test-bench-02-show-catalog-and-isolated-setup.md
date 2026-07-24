# 02 — Show catalog and isolated setup

## Outcome

Provide immutable canonical shows and isolated per-scenario working copies. Scenario setup becomes
one readable `show.use(...)` call, while the implementation guarantees that parallel tests never
mutate a shared show folder.

This step is fixture setup. Operator-facing create/load/save behavior is implemented in step 03.

## Public helpers

```ts
export enum Show {
  Empty = "empty",
  TwelveDimmers = "twelve-dimmers",
  CompactRig = "compact-rig",
  DefaultStage = "default-stage",
}
```

- `show.use(show)` — establish a fresh working copy as labelled setup;
- `show.expect.active(show)` — assert canonical identity plus working-copy metadata;
- `defineShow(name, recipe)` — register a reusable typed show recipe;
- `show.fixturePath(show)` — framework-internal fixture lookup, not exposed as a scenario escape
  hatch;
- `show.resetWorkingCopy()` — restore only the current test's copy.

## Catalog rules

- Canonical show inputs are read-only.
- Every scenario execution, surface, retry, and worker receives a unique data directory and
  working show identity.
- A recipe composes only public helpers completed by this or earlier steps.
- `show.use` reports that it is fixture setup and may use a fast harness path.
- A scenario that tests loading must use `show.load` from step 03 instead.
- Catalog entries declare fixtures, patch, modes, Groups, Presets, Cues, pages, and Desktops they
  promise so dependent scenarios can validate prerequisites early.
- A missing or stale catalog entry fails before the first operator action and prints available
  entries.

## Initial catalog

- `Empty` — no patched fixtures or stored objects;
- `TwelveDimmers` — the existing twelve Generic Dimmers;
- `CompactRig` — dimmers plus representative RGB and moving fixtures required by programmer and
  output tests;
- `DefaultStage` — the stable representative Stage/demo fixture.

Do not duplicate binary or large fixture data merely to create aliases. Catalog entries may point
to one canonical source and apply a typed setup recipe to the isolated copy.

## Helper-contract scenarios

1. `show.use(Show.Empty)` activates an isolated empty copy.
2. Two concurrent uses of `Show.CompactRig` receive different data paths and show IDs.
3. A mutation in one copy never appears in the other.
4. Reset restores the current copy without modifying the canonical source.
5. Catalog prerequisite validation reports missing fixture IDs, profiles, Groups, or Desktops.
6. Setup trace clearly distinguishes fixture creation from an operator action.
7. Cleanup removes only the test-owned temporary directory.
8. A failed scenario retains the configured diagnostic artifacts without making the next retry
   reuse its mutable show.

## Done gate

- Later helper tests can establish known state with one enum call.
- Parallel show setup is isolated and stress-tested.
- No scenario author supplies filesystem paths, raw show IDs, or revision plumbing.
- Canonical fixtures remain unchanged after the suite.

## Result

- Added the typed `Show` catalog, declarative `defineShow(...)` recipes, labelled `show.use(...)`
  fixture setup, semantic active-show expectations, and per-scenario working-copy reset.
- Canonical inputs remain immutable while every catalog use creates a unique working show inside
  the owning bench data directory. Output routes are rebound only in the working copy.
- Catalog prerequisites validate fixture numbers, profile names, and Groups before the first
  operator action. Desktop prerequisites deliberately fail with guidance to use
  `desktop.use(...)`, because Desktops are desk/user data rather than show objects.
- `BENCH-SHOW-001` through `BENCH-SHOW-003` cover the initial catalog, reset identity, stale
  prerequisites and diagnostics, concurrent bench paths and show IDs, mutation isolation,
  canonical preservation, setup tracing, and test-owned cleanup.
- Verified with `npm run test:architecture`, `npm run test:bench-types`, and the focused Playwright
  catalog `tests/testBench/02-show-catalog-and-isolated-setup.spec.ts` (3 passed). The full E2E
  catalog completed with 292 passed, 9 skipped, and one unrelated parallel-only `GROUP-004`
  failure; that exact case passed immediately in an isolated rerun.
