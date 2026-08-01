# Three-tier Demo and Benchmark Shows

## Usage Limits:
This plan should not get started if remaining usage is below 70%
If the usage drops below 60%, wrap up and give me a summary.

## Queue position and status

**Finished before final Stage optimization.**

**Completion usage gate: above 50% large-window remaining usage.**

**Claim checkpoint — 2026-07-29: 94% large-window remaining usage.**

Claim this plan only after Dynamics, Dedicated Virtual Playbacks and exclusion zones, and
repository-wide dead-code removal are finished. This phase builds and validates the three
deterministic workload shows and their benchmark harness. The following Stage plan optimizes the
renderer against them and owns the complete packaged performance sweep. Move this file to
`doing/` before changing implementation code and follow the workflow and usage gate in
[`../README.md`](../README.md).

The complete behavior and inventory contract is
[`docs/plans/Done/76-separate-demo-and-benchmark-shows.DONE.md`](../../Done/76-separate-demo-and-benchmark-shows.DONE.md).
This queue file makes that roadmap plan executable in the refactoring order; it does not itself
implement, generate, or benchmark a show.

This plan also owns completion of
[`49-product-demo-video-revision.DONE.md`](../../Done/49-product-demo-video-revision.DONE.md).
The generated Plan 76 demo and the maintained `DEMO-001` video must share one
authoritative show contract; do not build a second demo-only patch to satisfy the
recording.

## Pre-claim audit — 2026-07-29

The current maintained product-demo implementation is useful migration input but does
not satisfy this plan:

- `tests/support/plannedDemoState.ts` and
  `tests/bench/show/productDemoScenario.ts` currently assert 66 controllable fixtures
  and 114 physical instances, rather than 262 and 301.
- The current patch has two eight-lamp ACL controls (`ACL In` and `ACL Out`), while
  Plan 76 requires four named eight-lamp controls with distinct reviewed
  compositions.
- The current programming seed creates seven Color presets, five Position presets,
  no 30-Dynamic library, and a two-Cue `ACL Chase` assigned to Speed Group A. The
  replacement requires 13 Color presets, the complete Beam and seven-Position
  libraries, 30 Dynamics, and the four-Cue chase on Speed Group D.
- The current visible workflow saves a provisional `Demo Show`, uses repeated setup
  title cards, and shows a compact cue-programming result. Plan 49 requires one
  maintained polished recording with the revised setup, ACL spreading, output,
  fixture-control, preset, cue-programming, and centered-modal evidence.
- The sustained headless benchmark still exposes `--demo-show` terminology and must
  be renamed without changing its independent stress behavior.

The fourth ACL placement is approved as Front Split: four lamps left and four lamps
right on the Front Truss, fanning inward. Establish the deterministic generator and
manifest first, then migrate the maintained video and benchmark consumers onto their
separate authoritative tiers.

## Goal

Replace the ambiguous single demo/benchmark workload with three deterministic, independently
reported tiers:

1. The exact Plan 76 demo show—231 controllable fixtures plus 33 visual-only Venue records, for
   264 patch records and 306 physical Stage instances including multi-patches—exercised in the
   packaged desktop with 3D Stage and Fixture Sheet/list open. Its canonical Dynamic/Cuelist
   benchmark look, responsiveness, and end-to-end gates are release-blocking.
2. An exactly 1,000-fixture interactive benchmark with the same UI surfaces open. Report its
   achieved engine/output rate and Stage metrics against a 100 Hz target.
3. A 2,000-fixture headless stress benchmark that can scale to 4,000 fixtures, with every Stage and
   visualization UI surface disabled. Report its result against a non-blocking 60 Hz target.

The 1,000-fixture claim is the embedded-Stage support ceiling. Larger fixture counts are
headless-only evidence and must never be advertised as equivalent interactive Stage capacity.

## Workload contract

- Prefer representative higher-channel Profile, Beam, Wash, Sunstrip, and other production modes
  over a patch dominated by tiny RGB fixtures.
- Include at least 40 Showtec Sunstrip LED RGB fixtures in the 30-channel mode.
- Substantially occupy multiple universes without requiring every universe to end at channel 512.
- Run approximately 20 production Dynamic instances.
- Dynamically drive every applicable intensity and color value on Sunstrips and color-capable
  fixtures, and every applicable intensity, pan, tilt, and color value on moving fixtures.
- Partition Dynamic-driven fixtures intentionally and retain a separate fixed-dimmer population
  whose intensity has no Dynamic.
- In the headless tier, drive all compatible fixture values while leaving the Stage and
  visualization clients disabled.
- For the realistic tier, generate the exact 30-Dynamic library, four-Cue `ACL Chase`, Speed Group
  A–E mapping, and canonical active benchmark look defined by Plan 76. Do not add fixture filler
  to round the demo to an arbitrary count.

## Verification and reporting

Earlier queue phases run focused checks and their required end-to-end coverage. This plan proves
the generated inventories, canonical look, and harness behavior with focused checks. Defer the
complete packaged three-tier performance sweep until the following Stage optimization plan is
active.

When active:

1. Generate and validate all three shows independently.
2. Prove the realistic demo's canonical benchmark assignments and active look through the real
   show/runtime path.
3. Prove that the 1,000-fixture workload opens the Stage and Fixture Sheet/list surfaces required
   by the packaged benchmark.
4. Prove that the 2,000- and 4,000-fixture headless workloads disable Stage and visualization UI.
5. Prepare build-readable and human-readable report generation containing fixture count, profile/mode mix,
   Dynamic count, active UI surfaces, duration, hardware identity, achieved rate, work/latency
   percentiles, deadline misses, and blocking/non-blocking status.
6. Preserve the existing Stage latency, resource-stability, output-isolation, and supported
   platform gates for the final Stage phase. A fast engine rate alone is not proof of a responsive
   desktop.

Missing the headless 60 Hz target is a reported capacity result, not by itself a release failure.
Missing the realistic demo's UI acceptance is a release failure. The 1,000-fixture result must
report the 100 Hz target honestly and must not silently weaken output safety or visible Stage
acceptance to reach it.

Post-completion policy revision (2026-07-31): the 100 Hz value remains a visible reference, while
the public capacity indicator for the released 1,024-fixture engine/output proxy is green at
60 Hz or above, yellow at 40–59.999 Hz, and red below 40 Hz. This does not change the canonical
306-instance demo's 100 Hz packaged gate or the exact 1,000-instance UI requirement to keep Stage,
Fixture Sheet, and the rest of the desk responsive. The 2,000+ tier remains informational and now
reports its measured limiting phase.

## Completion

Before completion:

1. reconcile the generated workloads and harness against every acceptance item in Plan 76;
2. run the required focused and end-to-end generation/runtime checks, leaving the final packaged
   three-tier performance gate explicitly to the next Stage plan;
3. add a `## Result` section with generated artifacts, exact commands, report paths, hardware,
   measurements, limitations, and commit;
4. move this file to `finished/`; and
5. create the semantic commit required by the queue workflow.

## Result

Completed on 2026-07-29 with 91% large-window usage remaining.

### Generated tiers and runtime evidence

- The canonical demo generator produces exactly 231 controllable fixtures and adds 33 visual-only
  Venue records for 264 total patch records and 306 physical Stage instances. It retains 2,988
  occupied DMX slots, 12 layers, 35 Groups, 30 presets, 30 Dynamics, eight
  Cuelists, 14 Playbacks, Speed Groups A–E, and the reviewed four-set ACL layout. The focused
  `tests/76-demo-show-generation.spec.ts` runtime path passed with all 12 canonical benchmark
  assignments active.
- `tests/76-interactive-large-tier.spec.ts` passed in 28.3 seconds. It proves the exact
  970-record/1,000-instance interactive tier, 40 RGB Sunstrips, 20 active address-partitioned
  Dynamics, 440 fixed-control physical instances, 18,840 occupied slots across 37 universes, a
  visible 3D Stage, a simultaneously visible Fixture Sheet, and changing rendered frames.
- Short release-mode informational headless probes on an Apple M5 Max with 18 logical CPUs:
  - 2,000 fixtures: 60.20 achieved ticks/s at the 60 Hz target, zero dropped/deferred ticks and
    zero deadline misses, 11.60 ms total-pipeline p99, 82,001,920 resident bytes;
  - 4,000 fixtures: 39.63 achieved ticks/s, 20 dropped ticks, 38 deferred ticks, 40 deadline
    misses, 25.72 ms total-pipeline p99, 150,061,056 resident bytes. This tier is explicitly
    informational and non-blocking.

Headless reports:

- `.artifacts/performance/headless-stress-2000-2026-07-29T14-47-25-310Z.json`
- `.artifacts/performance/headless-stress-4000-2026-07-29T14-47-28-270Z.json`

### Maintained demo artifacts

`LIGHT_VISUAL_RECORDING=1 LIGHT_UPDATE_DEMO_SHOW=1 npm run test:demo` passed in 13.4 minutes and
refreshed the integrity-checked `assets/demo.show`. Visual inspection used:

- `.artifacts/test/visual-inspection/product-demo/tosklight-product-demo-h265.mp4` — HEVC,
  1920×1080, 25 fps, 793.8 seconds, 233,126,702 bytes;
- `.artifacts/test/visual-inspection/product-demo/tosklight-product-demo.webm` — 246,663,355 bytes;
- `.artifacts/test/visual-inspection/product-demo/tosklight-product-demo-1920x1080.png`; and
- `.artifacts/test/visual-inspection/product-demo/tosklight-product-demo-contact-sheet.png`.

### Verification

- `npm run test:e2e -- tests/76-demo-show-generation.spec.ts`
- `npm run test:e2e --workspace @tosklight/light-desktop -- tests/product-demo.spec.ts`
- `LIGHT_VISUAL_RECORDING=1 LIGHT_UPDATE_DEMO_SHOW=1 npm run test:demo`
- `npm run test:e2e --workspace @tosklight/light-desktop -- tests/76-interactive-large-tier.spec.ts`
- `node --test tools/stage-dynamics-scene.test.mjs tools/stage-large-scene.test.mjs`
- `cargo test -p light-headless-runtime dynamic_coverage --lib`
- `npx vitest run tests/bench/show/plannedDemoPlaybacks.test.ts tests/bench/show/plannedDemoDynamics.test.ts`
- product-demo mapping/control workspace tests, frontend build, Biome, source-size ratchet, semantic
  catalog generation, test-bench inventory generation, `git diff --check`, and SQLite
  `PRAGMA integrity_check`.

### Limitations and deferred gates

- The complete packaged three-tier duration sweep, the large end-to-end suite, the 100 Hz
  interactive rate report, and release-blocking renderer gates remain explicitly owned by Plan 21.
  A diagnostic packaged large-tier probe before the compact address partition exposed 13,080
  Programmer Dynamic values repeatedly embedding unrelated multi-lane definitions and growing
  persisted Programmer history beyond Node's JSON string ceiling. The workload now uses 20
  non-overlapping fixture/attribute partitions with compact single-lane targetless definitions;
  the bounded interactive acceptance passes. The full packaged rerun remains part of Plan 21.
- `Show LED Random Strobe` is active but the selected shipped Generic RGBW LED profile exposes no
  compatible `strobe` address; no unsupported channel is manufactured.
- The royalty-free Theater source has not been supplied. The Theater desktop persists, while
  script-specific Theater content remains explicitly pending under Plan 76.

### 2026-08-01 current canonical demo inventory

The later product-demo patch refinement supersedes the historical 262-control/301-lighting-
instance counts above. The current release-gated demo contains 231 controllable lighting
fixtures, 264 physical lighting instances, 33 visual-only Venue records, 264 total patch records,
306 physical Stage instances, and 2,988 occupied DMX slots. Its approximately 300-instance Stage
acceptance role is unchanged; the reduction allows Stage, Audience, and Auxiliary lighting to
remain in clear universe bands while universe 1 models the two conventional dimmer racks and
contains no movers.
