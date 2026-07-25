# 09 — Test recipes, reporting, and parallel hardening

## Outcome

Complete the reusable test-recipe layer, semantic failure reporting, reproducible route
selection, and parallel browser/server isolation before migrating the existing UI catalog.

## Test-recipe registry

Test recipes capture repeated operator intent, not arbitrary mechanics:

```ts
defineTestRecipe("record-two-cue-dimmer-show", async (t, options: {
  playback: number;
  selection: SelectionChunk[];
}) => {
  await t.selection.targets(...options.selection);
  await t.encoder.intensity.dimmer.set(25);
  await t.record.cue({ playback: options.playback, cue: 1 });
  await t.encoder.intensity.dimmer.set(100);
  await t.record.cue({ playback: options.playback, cue: 2 });
});

await t.recipe.run("record-two-cue-dimmer-show", {
  playback: 1,
  selection: [fixtureRange(1, 6)],
});
```

Rules:

- typed, discoverable registry;
- public semantic helpers only;
- names describe achieved state;
- supported routes derive from contained actions;
- expanded semantic steps remain visible on failure;
- `ensure`/`use` setup may be idempotent;
- destructive operator workflows remain explicit;
- no `page`, `locator`, `fetch`, filesystem, or raw show-object access.

Initial test recipes should cover only demonstrated repetition:

- use a registered Desktop;
- create/load a canonical show;
- establish a representative fixture selection;
- record a two-Cue dimmer look;
- establish representative output routes;
- open a typed pane and return its handle.

## Route reporting

When no explicit route is requested:

1. generate or accept `LIGHT_TEST_ROUTE_SEED`;
2. derive the action choice from seed, scenario ID, coverage run, retry, and action index;
3. record candidates and selected route as a Playwright step;
4. print a concise normal-run summary;
5. attach the complete trace on failure.

Example:

```text
SELECT · fixtures 1 THRU 5 · route=fixture-sheet-touch · seed=9f53a1
ENCODER · intensity.dimmer.add(3) · route=touch-fader · seed=9f53a1
```

## Synchronization and failure evidence

Every action waits for authoritative evidence, not arbitrary sleeps:

- command/session acknowledgement;
- show/pane/Programmer projection;
- logical frame timestamp;
- OSC feedback sequence;
- packet newer than a mark;
- visible modal/progress state.

On failure attach:

- semantic action trace and route seed;
- application screenshot and relevant pane screenshots by slug;
- browser video when enabled;
- active show and Desktop identity;
- selected fixtures and Programmer summary;
- latest logical DMX frame and wire packet marks where relevant;
- server log tail and audit records.

## Parallel hardening

Preserve one `LightBench` per scenario execution:

- unique temporary data/show directory;
- independent HTTP, OSC, Art-Net, and sACN ports;
- fresh session, Programmer, virtual clock, and observers;
- artifacts namespaced by scenario, route, worker, retry, and surface.

Before raising worker count:

- remove free-port handoff races by server-selected port `0` or retained reservations;
- prohibit shared mutable output paths;
- keep canonical show fixtures read-only;
- keep manual screenshot refresh, video assembly, and demo-show refresh serial;
- prove deterministic process-tree cleanup.

This remains browser Playwright. Native Tauri instances and OS-window control are out of scope.

## Helper-contract scenarios

1. Test-recipe expansion reports each contained semantic action.
2. Test-recipe route capabilities reject an unsupported explicit route before mutation.
3. A fixed route seed reproduces every selected adapter.
4. A changed seed rotates eligible UI paths.
5. Failure evidence names the intended target and last authoritative observation.
6. Start more benches than the default worker count and prove no cross-instance show, session,
   command-line, DMX, OSC, port, slug-registry, or artifact leakage.
7. Crash one bench and prove cleanup does not stop or delete another bench.
8. Run product-demo free run while ordinary tests use deterministic clocks.

## Done gate

- A failure can be replayed from its seed and diagnosed from attached evidence.
- Four or more ordinary benches pass the isolation stress gate.
- Test recipes reduce repetition without hiding selectors or raw transport calls.
- The bench is ready to express existing UI acceptance tests.

## Result

Completed on 2026-07-25.

- Added a typed, discoverable recipe registry whose implementations receive only the
  public semantic Desktop, Show, selection, encoder, Record, and pane helpers. The
  initial recipes cover registered Desktops, canonical working shows, representative
  selection and output setup, two-Cue dimmer recording, and typed pane creation.
- Unsupported explicit recipe routes are rejected before a recipe can mutate desk
  state. Recipe reports retain every expanded semantic step emitted by the contained
  helpers.
- Every browser scenario now derives all unqualified route choices from
  `LIGHT_TEST_ROUTE_SEED` (or a generated seed), project/scenario identity, retry, and
  each helper's action index. A concise seed/action summary is printed and the full
  candidate/selection trace is attached as JSON.
- Failure evidence now adds the semantic action trace, route seed, error, active
  show/Desktop/session identity, selection and Programmer observations, and an
  application screenshot. The existing fixture layer continues to attach server,
  audit, OSC, logical-DMX, wire-packet, and virtual-clock evidence.
- The parallel stress contract starts five additional ordinary benches at once,
  verifies unique data directories and HTTP/OSC/Art-Net/sACN ports, distinct shows
  and sessions, isolated command lines, concurrent free-run/deterministic clocks,
  and survival of one bench after another is killed and cleaned up.

Verification:

- focused recipe and five-bench parallel scenarios: 2 passed;
- focused reruns of all full-suite timing failures: 5 passed and 62 passed;
- Control UI typecheck and bench typecheck: passed;
- architecture, command-boundary, source-size, and diff checks: passed;
- full browser E2E exercised all 336 tests twice. The runs completed at
  325 passed / 9 skipped and 324 passed / 9 skipped; their unrelated timing
  failures all passed in the corresponding focused reruns above.
