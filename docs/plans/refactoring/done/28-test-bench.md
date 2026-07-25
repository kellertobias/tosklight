# 28 — Test bench

## Goal

Build the operator-facing Playwright test bench incrementally. Each numbered plan introduces one
coherent helper family, proves that family with focused helper-contract scenarios, and leaves the
bench usable before the next step begins. Existing UI tests migrate only after the helper catalog
is complete enough to express them without selectors, HTTP paths, pointer coordinates, or
knowledge of application internals.

This is a browser Playwright framework. It does not drive or claim coverage of packaged Tauri
webviews or native OS windows.

## Locked authoring contract

Scenarios are one linear async script with any number of action/expectation sequences:

```ts
scenario("DIMMER-101", "Record and run two looks", async (t) => {
  await t.show.use(Show.CompactRig);
  await t.desktop.use(Desktop.Programming);

  await t.selection.fixtures.range(1, 5);
  await t.encoder.intensity.dimmer.set(50);
  await t.record.playback(1);
  await t.clock.advanceBy("3s");
  await t.expectFixtureDMX(fixtureRange(1, 5), {
    "Dimmer coarse": 128,
  });

  await t.encoder.intensity.dimmer.set(100);
  await t.record.cue({ playback: 1, cue: 2 });
  await t.playback.go(1);
  await t.clock.advanceBy("3s");
  await t.expectFixtureDMX(fixtureRange(1, 5), {
    "Dimmer coarse": 255,
  });
});
```

There is no object-shaped `given`/`when`/`then` structure and no restriction to a single action
or expectation phase.

### Route selection

Every multi-path action supports an unqualified seeded choice and explicit `.via.*` routing:

```ts
await t.selection.fixtures.range(1, 5);
await t.selection.fixtures.via.touch.range(1, 5);
await t.selection.fixtures.via.ui.range(1, 5);
await t.selection.fixtures.via.api.range(1, 5);
await t.selection.fixtures.via.osc.range(1, 5);
```

The unqualified route is pseudo-random but reproducible. Reports contain the chosen route and
seed, and `LIGHT_TEST_ROUTE_SEED` replays it. The framework creates API coverage only where an
independent API operation is meaningful. UI-only presentation behavior never receives a
decorative API test.

When click modifiers are themselves the contract, the semantic action exposes routes such as
`.via.click` and `.via.shiftClick`. There is no general locator-based click escape hatch.

### Setup versus behavior under test

- `show.use(...)`, `desktop.use(...)`, and registered setup recipes establish isolated fixtures.
- `show.load(...)`, `desktop.configure(...)`, and ordinary actions exercise the selected
  production route.
- Setup may be accelerated only when the report labels it as setup and the behavior is not under
  test.
- UI actions use visible production controls. They must not silently mutate state through an API.

### Stable semantic identities

- Shows and registered Desktops use enums or typed catalog entries.
- Panes use `PaneType` plus a required stable slug.
- Encoder operations use enum-backed group and attribute targets.
- Fixtures, Groups, Cues, Playbacks, pages, screens, and test-recipe parameters use typed
  identities.
- Scenario files never use runtime UUIDs, CSS selectors, raw route strings, raw pane IDs, or
  physical encoder slot numbers.

## Implementation sequence

Implement in this order. A later step may use only helpers completed by earlier steps.

| Step | Plan | Outcome |
| --- | --- | --- |
| 01 | [Browser UI and Desktop panes](28-test-bench-01-browser-ui-and-desktop-panes.md) | Minimum scenario world, browser access, semantic UI seams, typed Desktop builder, actionable pane handles, screenshots. |
| 02 | [Show catalog and isolated setup](28-test-bench-02-show-catalog-and-isolated-setup.md) | Immutable canonical shows, per-test working copies, `show.use`, setup recipes, cleanup. |
| 03 | [Show operator workflows](28-test-bench-03-show-operator-workflows.md) | Create, load, save, revisions, restart, malformed/legacy recovery through truthful routes. |
| 04 | [Clock, DMX, and output observations](28-test-bench-04-clock-dmx-and-output.md) | One-step, duration, awaited free run, fixture-aware DMX, raw/wire observations. |
| 05 | [Command line and selection](28-test-bench-05-command-line-and-selection.md) | Typed selection chunks, Highlight, next/previous/all, and modifier-aware UI, touch, API, and OSC routes. |
| 06 | [Programmer and encoders](28-test-bench-06a-normalized-programmer-encoders.md) | Programmer Fade, enum-backed encoder tree, central page/slot resolver, typed value expressions. |
| 07 | [Groups and Presets](28-test-bench-07-groups-and-presets.md) | Store, recall, update, delete, ordered membership, empty/absent semantics. |
| 08 | [Cues, Playbacks, pages, and Preload](28-test-bench-08-cues-playbacks-pages-and-preload.md) | Cue Fade, Speed Groups with humanized tap tempo, recording/runtime helpers, explicit addressing, and press/release behavior. |
| 09 | [Test recipes, reporting, and parallel hardening](28-test-bench-09-test-recipes-reporting-and-parallelism.md) | Reusable operator test recipes, traces, artifacts, seeded routes, isolated parallel benches. |
| 10 | [Existing UI-test migration](28-test-bench-10-existing-ui-test-migration.md) | Incremental migration and retirement of old author-level helpers. |

## Rules for every helper-building step

Steps 01–09 must:

1. define the public helper types before implementing adapters;
2. add focused helper-contract tests that use the public API exactly as a scenario author would;
3. prove visible UI actions through the real browser application;
4. prove API, OSC, wire, or harness paths only when the helper declares those capabilities;
5. attach semantic steps and useful evidence on failure;
6. keep existing tests runnable;
7. update `docs/testing/README.md` with only the newly available authoring surface;
8. stop at its done gate before beginning the next numbered plan.

Helper-contract tests are not throwaway unit tests. They are small Playwright scenarios that
prove the helper reaches the intended production surface and that its semantic oracle observes
the correct result. Lower-level unit/type tests may supplement them.

## Shared architecture

The completed bench has four layers:

1. **Scenario world** — the small typed API imported by test authors.
2. **Semantic intents** — Desktop, show, selection, encoder, record, playback, clock, output, and
   assertion contracts without Playwright or HTTP details.
3. **Route adapters** — visible UI, touch, typed API, OSC, harness, and wire implementations.
4. **Normalized observations** — shared state and output projections used by assertions.

Every intent declares its supported routes. Unsupported explicit routes fail before performing
the first action; there is no silent fallback.

## Final completion criteria

- A maintainer can author representative show, Desktop, selection, encoder, Group, Preset, Cue,
  Playback, DMX, time, screenshot, and demo-recording scenarios using only semantic helpers.
- The helper-contract suite proves every public helper family independently.
- An unqualified multi-route action reports its reproducible choice and seed.
- Four or more browser/server benches run concurrently without port, show, session, command-line,
  DMX, OSC, or artifact leakage.
- Existing UI scenarios migrate incrementally and retain their operator-visible acceptance
  contracts.
- No migrated scenario contains application selectors, raw API paths, fixture UUID lookups, grid
  pointer math, physical encoder slots, or modal click recipes.
- The framework never implies packaged Tauri coverage.

## Result

All helper-building and migration steps are complete. The bench is organized by operator area,
the root migration inventory covers 309 cases with no pending rows, and the final authoring
contract lives in `docs/testing/README.md`. Detailed implementation and verification results are
recorded in the linked step plans and the subsequent `28-test-bench-*` files in this directory.
