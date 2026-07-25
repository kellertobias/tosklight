# End-to-End Test Author Guide

Executable acceptance coverage lives in the repository-root `tests/` directory.
The Markdown files beside this guide define the operator contract; the generated
[semantic catalog](../engineering/semantic-test-catalog.html) shows how marked
scenarios exercise it.

## Choose the truthful test shape

- Use `scenario(...)` for operator-visible browser behavior that can be written
  entirely through the public semantic world.
- Keep an `@api` test only when the behavior cannot be driven truthfully
  through production UI: raw transport contracts, constructed failure modes,
  persisted corruption, migration/restart boundaries, concurrency, or exact
  wire timing. Do not repeat an operator workflow through the API when its UI
  scenario already verifies the same authoritative state.
- Treat OSC and attached hardware as UI input surfaces and cover them in an
  `@ui` semantic scenario. Use `@desktop` only for packaged-app lifecycle
  behavior that requires a native bundle.
- Never edit the canonical shows. `show.use(...)` creates an isolated working
  copy with a fresh session, Programmer, virtual clock, receivers, and OSC
  subscriptions for every scenario.

## Semantic scenario template

```ts
// @bench-semantic-world

import { scenario } from "../tests/bench/core/scenario";
import { fixtureRange } from "../tests/bench/output/fixtureDmxContract";
import { Show } from "../tests/bench/show/showScenario";

scenario("DIMMER-101", "Record and run a look", async (t) => {
	await t.show.use(Show.CompactRig);
	await t.app.open();
	await t.app.expect.ready();

	await t.selection.fixtures.range(1, 5);
	await t.encoder.intensity.dimmer.via.ui.set(50);
	await t.record.playback(1);
	await t.clock.advanceBy("3s");

	await t.expectFixtureDMX(fixtureRange(1, 5), { Intensity: 128 });
});
```

A marked file may use only public world calls. It must not import Playwright
`Page` or `Locator`, `ApiDriver`, support-catalog mutation helpers, raw HTTP
paths, selectors, coordinates, runtime UUIDs, physical encoder slots, or
mutable show-object internals.

## Authoring rules

- Write one linear async script. Any number of action and expectation phases is
  valid; there is no `given`/`when`/`then` object shape.
- Establish fixtures with `show.use(...)`, registered recipes, and typed
  Desktop configuration. Use `PaneType` plus a stable kebab-case slug for
  panes.
- Name Fixtures, Groups, Presets, Cues, Playbacks, Pages, screens, encoder
  groups/attributes, and special controls through their typed semantic
  identities.
- Use unqualified actions when seeded route diversity is useful. Use
  `.via.ui`, `.via.touch`, `.via.api`, `.via.osc`, or another declared route
  when the surface is part of the contract. Unsupported explicit routes must
  fail before mutation.
- `LIGHT_TEST_ROUTE_SEED=<seed>` replays unqualified choices. Failure artifacts
  include the chosen routes, semantic steps, show/Desktop/session identity,
  state observations, screenshots, and server/protocol evidence.
- UI actions use visible production controls. API setup is allowed only when it
  is labelled setup and is not the behavior under test.
- Keep programmer values LTP unless a named playback path specifies otherwise.
  Preserve ordered selection and Group membership whenever spreading or
  operator intent depends on order.
- Distinguish current-page Playback addresses from explicit-page addresses.
  Attached OSC hardware joins one desk’s command line and interaction state;
  another desk alias remains isolated.
- Advance lighting time with the virtual clock. Wall time is only for browser
  mechanics such as a long press or process-start deadline.
- Synchronize on revisions, visible state, OSC feedback, audit events, or
  packets newer than a recorded mark. Do not use sleep as proof of completion.

## Main world surfaces

The public world includes application/show/Desktop lifecycle, command line and
keypad, selection and Highlight, Programmer and encoder controls, Groups and
Presets, Cues and Playbacks, Pages and Preload, Speed Groups and timing, Patch,
DMX/output observations, screens, files/text editing, desk lock, operator-shell
contracts, attached hardware, and product-demo/recording helpers.

Prefer an existing helper. If a new operator intent is required:

1. add one narrow public method in the area-owned bench folder;
2. keep selectors, transport paths, and browser mechanics inside its adapter;
3. add the method to the narration catalog;
4. prove it with a focused marked scenario;
5. preserve any genuinely low-level companion boundary;
6. regenerate and check the inventory and semantic catalog.

## Verification

Start focused, then widen according to risk:

```sh
npm run test:unit
npm run test:e2e-api
npm run test:e2e-ui
npm run test:e2e -- tests/<focused-spec>.spec.ts
node tools/check-architecture.mjs
node tools/check-source-size.mjs
node tools/test-bench-migration-inventory.mjs --check
npm run test:semantic-test-docs
```

Use `./test record` for the narrated visual catalog and `./test demo` for the
maintained product walkthrough. Browser Playwright coverage does not claim
packaged Tauri or native OS-window coverage.

Regenerate maintained indexes after scenario changes:

```sh
node tools/test-bench-migration-inventory.mjs --write
npm run docs:semantic-tests:write
```

The inventory must end with no `pending-semantic-migration` rows. Generated
documentation must be deterministic and checked in only when the repository
workflow calls for it.
