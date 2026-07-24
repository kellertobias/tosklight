# 04 — Clock, DMX, and output observations

## Outcome

Add deterministic time controls, genuine awaited free run for recorded demos, fixture-aware DMX
assertions, and explicitly low-level logical/wire observations. These become stable oracles for
the programming and playback helpers in later steps.

## Clock helpers

The three primary operations are deliberately distinct:

```ts
await t.clock.advanceStep();
await t.clock.advanceBy("2.5s");

// Existing Playwright video capture remains active while this promise is pending.
await t.clock.freeRunFor("8s");
```

- `advanceStep()` advances exactly one deterministic engine step and renders the result.
- `advanceBy(duration)` advances virtual application time by the requested duration and renders
  the result.
- `freeRunFor(duration)` starts the genuine scheduler, awaits the wall-clock duration while
  effects and browser rendering remain live, stops free run, and then resolves.
- `clock.at(checkpoints, callback)` evaluates exact named boundaries.
- `clock.waitWall(duration)` is reserved for browser gesture mechanics such as a long press; it
  never represents lighting time.

`freeRunFor` must not be a Playwright timeout while the engine remains frozen. It is the operation
used by the browser product-demo recording when live effects must appear in captured frames.

## Fixture-aware DMX

Normal scenarios address fixture identity and actual profile channel-component names:

```ts
await t.expectFixtureDMX(fixture(101), {
  "Pan coarse": 127,
  "Pan fine": { between: [0, 255] },
  "Pan ultra": { between: [0, 31] },
  "Color red": 255,
  "Color green": { between: [0, 16] },
});
```

```ts
type ExpectedDMXByte = number | { between: readonly [number, number] };
```

The resolver uses the fixture's current patch, profile mode, logical head or multipatch, and
named raw channel component at assertion time. Repatching therefore changes the inspected
universe/address without changing the scenario.

Rules:

- exact bytes and inclusive range endpoints are integers from 0 through 255;
- names normalize only case and surrounding whitespace;
- missing or ambiguous names fail with valid profile channel names;
- coarse, fine, and ultra names remain separate raw bytes;
- fixture collections apply the same assertion to every member;
- an unpatched fixture fails as having no DMX assignment unless absence is explicitly asserted;
- assertions inspect the latest frame and never advance time implicitly;
- diagnostics include fixture, profile, mode, head/multipatch, universe, address, actual byte,
  accepted value/range, and timestamp.

## Low-level observations

Keep these for tests where layout or protocol is itself the contract:

- `dmx.frame(universe)`;
- `expect.dmx(universe).channel(address, value)`;
- `expect.dmx(universe).channels(map)`;
- `expect.dmx(universe).range(start, values)`;
- `expect.outputPacket(protocol, universe, assertion)`;
- `dmx.waitFor(...)`;
- global output controls for grand master, blackout, and paused dynamics.

Logical DMX, Art-Net, and sACN are distinct assertions. A combined helper must say explicitly
that it verifies all layers.

## Helper-contract scenarios

1. One step produces exactly one new deterministic frame.
2. Duration advancement hits exact transition boundaries without wall waiting.
3. Free run keeps a visible dynamic effect moving for the awaited recording interval and stops
   before the next action.
4. Fixture-aware exact and `between` assertions resolve a coarse/fine/ultra profile.
5. Repatch a fixture and prove the unchanged semantic assertion follows it.
6. Assert a typed fixture range.
7. Report missing channel, ambiguous channel, invalid byte/range, and unpatched fixture clearly.
8. Prove raw logical-frame, Art-Net, and sACN assertions do not masquerade as each other.
9. Polling failures attach the last frame and current application timestamp.

## Done gate

- Later scenarios never need hard-coded addresses unless patch or routing is under test.
- Exact transition tests are deterministic.
- Demo video can show genuinely free-running browser effects.
- Time and output failure evidence is sufficient to diagnose the last observed state.

## Result

- Added strict application-clock helpers for one deterministic step, exact duration jumps, named
  absolute checkpoints, wall-only gesture waits, and bounded free run. Free run drives the
  production deadline scheduler against the manual clock, serializes with deterministic clock
  operations, renders the exact final timestamp, and is exposed only by the loopback test bench.
- Migrated the product-demo recording's moving intervals to genuine free run while retaining fast
  deterministic jumps outside recording mode.
- Added semantic fixture and fixture-range handles, exact/current profile-mode resolution,
  head/multipatch qualification, coarse/fine/ultra byte assertions, explicit absent-patch
  assertions, repatch-safe snapshot reads, and actionable profile/address diagnostics carrying
  the last application timestamp.
- Added separate latest logical-DMX, polling, Art-Net, and sACN observations plus typed global
  Grand Master, blackout, and configured Pause Dynamics actions. Logical assertions never tick
  the clock or masquerade as wire assertions.
- Replaced packet-history array indexes with monotonic receiver cursors, so marks remain valid
  when the bounded 200-packet diagnostic history rotates. Failure artifacts now include the last
  logical DMX snapshot alongside virtual time and decoded wire packets.
- Added `BENCH-CLOCK-DMX-001` and `BENCH-CLOCK-DMX-002` for exact boundaries, semantic fixture
  lookup/ranges/errors, a changing live Phaser across both protocols, and proof that output stops
  before the next action.
- Split the fixture contract, resolver, and polling assertion so no new file exceeds the 400-line
  design goal.

Verification:

- focused test-bench Vitest: 32 passed.
- focused clock/DMX browser and wire scenarios: 2 passed.
- server production-route gating test: passed.
- `npm run test:architecture`: passed; source-size goals remain at 156 files over 400 lines.
- `npm run test:bench-types` and control-UI typecheck: passed.
- full `npm run test:e2e`: 297 passed / 9 skipped / 1 unrelated HIGHLIGHT-003 UI failure;
  the exact HIGHLIGHT-003 case passed immediately in isolation. Both new scenarios and the
  product-demo scenario passed in the full run.
