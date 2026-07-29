# Three-tier Demo and Benchmark Shows

## Usage Limits:
This plan should not get started if remaining usage is below 70%
If the usage drops below 60%, wrap up and give me a summary.

## Queue position and status

**Pending after repository-wide dead-code removal and before final Stage optimization.**

**Completion usage gate: above 50% large-window remaining usage.**

Claim this plan only after Dynamics, Dedicated Virtual Playbacks and exclusion zones, and
repository-wide dead-code removal are finished. This phase builds and validates the three
deterministic workload shows and their benchmark harness. The following Stage plan optimizes the
renderer against them and owns the complete packaged performance sweep. Move this file to
`doing/` before changing implementation code and follow the workflow and usage gate in
[`../README.md`](../README.md).

The complete behavior and inventory contract is
[`docs/plans/Next/76-separate-demo-and-benchmark-shows.md`](../../Next/76-separate-demo-and-benchmark-shows.md).
This queue file makes that roadmap plan executable in the refactoring order; it does not itself
implement, generate, or benchmark a show.

This plan also owns completion of
[`49-product-demo-video-revision.md`](../../Next/49-product-demo-video-revision.md).
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

Do not mutate this compact implementation piecemeal before the fourth ACL placement
is approved. Once this plan is claimed, establish the deterministic generator and
manifest first, then migrate the maintained video and benchmark consumers onto their
separate authoritative tiers.

## Goal

Replace the ambiguous single demo/benchmark workload with three deterministic, independently
reported tiers:

1. The exact Plan 76 demo show—262 controllable fixtures and 301 physical Stage instances,
   including multi-patches—exercised in the packaged desktop with 3D Stage and Fixture Sheet/list
   open. Its canonical Dynamic/Cuelist benchmark look, responsiveness, and end-to-end gates are
   release-blocking.
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

## Completion

Before completion:

1. reconcile the generated workloads and harness against every acceptance item in Plan 76;
2. run the required focused and end-to-end generation/runtime checks, leaving the final packaged
   three-tier performance gate explicitly to the next Stage plan;
3. add a `## Result` section with generated artifacts, exact commands, report paths, hardware,
   measurements, limitations, and commit;
4. move this file to `finished/`; and
5. create the semantic commit required by the queue workflow.
