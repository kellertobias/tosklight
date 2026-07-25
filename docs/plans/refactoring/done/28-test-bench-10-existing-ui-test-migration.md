# 10 — Existing UI-test migration

## Outcome

Migrate existing repository-root UI scenarios to the completed semantic bench incrementally.
Preserve their acceptance contracts while removing author-level selectors, raw transports, and
duplicated interaction recipes.

Do not begin this step until steps 01–09 meet their done gates. Missing helper capability is fixed
in its owning numbered step before migrating a scenario that needs it.

## Inventory

Create a migration table for every root `tests/*.spec.ts` UI case:

- scenario ID and help/testing contract;
- current setup fixtures;
- UI interactions and assertions;
- required new helpers;
- API/OSC/wire relevance;
- screenshots/video/generated artifacts;
- serial or parallel constraints;
- migration status.

Classify existing helpers:

1. keep as internal adapter implementation;
2. wrap temporarily for compatibility;
3. replace with a public semantic helper;
4. remove after the last consumer.

## Migration order

Migrate by complete representative workflow rather than file size:

1. one Desktop/pane and screenshot scenario;
2. one show create/load/save scenario;
3. one deterministic time and fixture-aware DMX scenario;
4. one command/selection scenario;
5. one Programmer/encoder spread scenario;
6. one Group and one Preset scenario;
7. one two-Cue Playback/page scenario;
8. one OSC parity scenario;
9. product-demo/video free-run path;
10. remaining scenarios grouped by helper family.

After each representative migration, pause and improve the public helper if the scenario remains
hard to read. Do not work around a missing abstraction with a local locator or raw API call.

## Per-scenario acceptance review

For every migrated scenario:

- compare against its exact `docs/testing` contract;
- preserve exact operator wording, mode, route, page addressing, and gestures;
- retain independent UI/API variants only where meaningful;
- keep screenshot names and generated-document workflow stable where required;
- verify that unpatched fixtures, empty Groups, ordered selection, Programmer LTP, and
  Playback/Cue HTP semantics remain correct where applicable;
- confirm the action trace describes intent rather than implementation;
- delete old local helpers only after proving no remaining consumers.

## Architecture enforcement

Add or extend architecture checks so migrated scenario files cannot import:

- Playwright `Page`/`Locator` for application interaction;
- low-level `ApiDriver` or raw HTTP clients;
- application reducer actions;
- internal pane IDs or CSS selectors;
- fixture UUID resolution helpers;
- pointer/grid coordinate click utilities;
- encoder slot helpers;
- raw mutable show-object writers.

Explicit exceptions require a narrow comment and apply only when the low-level mechanism itself
is the acceptance contract, such as protocol encoding.

## Verification cadence

Start with the focused migrated scenario, then its surface runs, then the full browser suite:

```sh
npm run test:architecture
npm run test:unit
npm run test:e2e -- tests/<migrated-focused-spec>.spec.ts
npm run test:e2e-api -- --grep '<scenario-id>'
npm run test:e2e-ui -- --grep '<scenario-id>'
npm run test:e2e-supplemental -- --grep '<scenario-id>'
npm run test:e2e
```

Refresh help screenshots or demo artifacts only when that migration intentionally changes their
authoritative generated output. Review visual differences rather than accepting them blindly.

## Done gate

- Every existing UI scenario is migrated or has a documented, justified low-level boundary.
- Scenario files read as linear operator intent and use the shared semantic world.
- Old compatibility wrappers and orphaned helpers are removed.
- The architecture check prevents regression to selectors and raw transports.
- The full browser suite and parallel isolation stress gate pass.
- `docs/testing/README.md` is the concise, current author guide.

## Result

Split on 2026-07-25 after re-inventorying the live catalog: the repository has 39
root Playwright spec files and 336 current cases, so migrating them as one commit
would make helper corrections and acceptance review unsafe.

Execution continues in:

- `10a-existing-ui-test-inventory-and-enforcement.md`;
- `10b-representative-ui-workflow-migration.md`;
- `10c-remaining-ui-family-migration-and-cleanup.md`.

The children retain this plan's acceptance criteria and order. This parent records
only the split and does not claim that any existing scenario has been migrated.
