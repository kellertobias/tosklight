# Supported Scale, Output Isolation, and Warm Operator UI

## Status

**IN PROGRESS — claimed 2026-08-01.** Its prerequisite Stage visualizer, operator-polish, and
Attribute Registry plans are complete. This plan now owns the mandatory packaged 1,000-instance
benchmark and any repairs it proves necessary before
[Macros](../pending/32-macros-and-scheduled-macros.md).

## Retained baseline and repair progress

- Commit `a338e93a` added the permanent `benchmark:supported-scale` packaged profile before
  production changes. Its first run retained
  `.artifacts/performance/stage/packaged-tauri-supported-scale-2026-08-01T14-50-06-480Z.json`.
  The exact 970-control/1,000-physical-instance scene prepared successfully, then the production
  configuration rejected 60 Hz because both validation and the scheduler were limited to 40–44
  Hz.
- Commit `a5f4d016` widened the production configuration, scheduler, Desk Setup field, and Help
  contract to 40–60 Hz while retaining the 44 Hz default. Focused Rust, UI, type, formatting, and
  boundary checks passed.
- The post-repair run retained
  `.artifacts/performance/stage/packaged-tauri-supported-scale-2026-08-01T15-08-23-965Z.json`.
  It achieved 59.93 Hz and all 59 sampled Programmer actions met the two-tick rule. It also proved
  that the current harness is not yet this plan's required workload: it sends zero network packets,
  shows two in-window Stage surfaces, races its optional sibling Stage opening against the 30-second
  Stage-window finish, and contains no correlated Playback UI/OSC matrix or Fixture Sheet value
  convergence gate. The production scheduler recorded two no-Stage misses and one Stage-window
  miss; the Stage-window p99 remained 12 ms in both windows.
- The corrected literal surface/network run retained
  `.artifacts/performance/stage/packaged-tauri-supported-scale-2026-08-01T15-40-30-479Z.json`.
  It ran one 3D Stage beside one Fixture Sheet, removed unrelated sibling-window, context-loss,
  active-show-switch, and application-suspend stress, and captured actual Art-Net plus sACN UDP
  packets. The control window had zero deadline misses; the Stage window delivered 3,666 packets
  at 61.1 measured frames/s with zero send errors and unchanged 12 ms p99. One 21.98 ms tick missed
  its deadline. Production phase logging attributed 18.20 ms of that tick to engine evaluation,
  3.49 ms to Dynamic sampling, 112 µs to publication, and 71 µs to network send. This is the first
  genuine retained hard-gate failure from the corrected workload.
- The first measured engine repair caches immutable split footprints in the compiled fixture-mode
  encoding plan and renders directly from borrowed root and multipatch destinations instead of
  allocating a footprint map and patch vector for every schema-v2 fixture on every output tick.
  The same corrected workload retained
  `.artifacts/performance/stage/packaged-tauri-supported-scale-2026-08-01T15-52-36-021Z.json`.
  It sustained 60.8 measured frames/s and 3,648 real network packets in the Stage window, with
  zero send errors and the same 12 ms p99. The worst tick fell from 22.03 ms to 17.32 ms, but one
  Stage-window deadline miss remains; the exact zero-miss gate is therefore still failing. All
  102 Programmer actions remained within two ticks. Focused fixture encoding and schema-v2 engine
  tests pass.
- Playback actions now enter the same bounded causal timing resource at authenticated WebSocket,
  HTTP, and OSC boundaries. Go, Flash press, Flash release, and master actions retain distinct
  labels, wake the production scheduler after successful authoritative handling, and are credited
  only after a later production network-send boundary completes. Focused route and OSC/WebSocket
  classification tests pass. The packaged workload still has to exercise and gate the complete
  UI/OSC matrix before this requirement is proven.

Plan 31 therefore remains on the repair path. Do not interpret the sibling-window failures as a
product acceptance failure: this plan requires one 3D Stage. The next slice must eliminate the
remaining engine-evaluation outlier while completing the still-missing correlated Playback UI/OSC matrix,
representative Cuelist/fade workload, and Fixture Sheet convergence evidence.

This plan turns the root
[`README.md`](../../../../README.md#supported-usage-profile) contract into measured packaged
behavior. All discussed efficiency and capacity improvements remain in this one issue until the
mandatory first benchmark below is complete. Do not split or defer them in advance.

## Goal

Prove or restore the supported **1,000-fixture at 60 Hz** operator path with all of these active at
the same time:

- the exact maintained 1,000-physical-instance benchmark show;
- one built-in 3D Stage view;
- one Fixture Sheet;
- the bundled Playback and command surfaces;
- representative running Cuelists, Cue fades, Playback fades, and Dynamics; and
- actual configured Art-Net and/or sACN output from the production server scheduler.

The Stage may stutter at this scale. It must stay bounded and recoverable and may not transfer its
cost to output, action handling, Playback feedback, Fixture Sheet, or other desk surfaces.

## Mandatory first check

Before production changes, add and run one deterministic packaged benchmark which:

1. loads and activates the exact 1,000-instance show;
2. configures the production output scheduler to 60 Hz;
3. opens one 3D Stage and one Fixture Sheet in the bundled desktop;
4. starts representative Cuelists, fades, Playbacks, and Dynamics;
5. warms the application and measures a sustained window;
6. exercises the action matrix below through both bundled UI and OSC;
7. records server receipt, authoritative mutation, output tick, production network send, client
   feedback, Fixture Sheet convergence, queue depth, payload, and resource evidence; and
8. retains raw machine-readable evidence under `.artifacts/performance`.

The test must use the production packaged path and scheduler. A headless engine loop,
Chromium-only Playwright result, mocked OSC call, simulated React component, or direct application
service call does not replace it.

After it is stable, this test becomes part of the regular performance benchmark suite and remains
committed whether the first run passes or exposes required work.

## Required action matrix

At minimum, exercise:

- **Go** to the next Cue from the visible bundled Playback UI;
- **Go** to the next Cue through OSC;
- Playback **Flash press** and **Flash release** from the bundled UI;
- Playback **Flash press** and **Flash release** through OSC; and
- one representative visible Playback fader or level change through each applicable UI and OSC
  route.

Each sample needs one correlation identity from its input boundary through client feedback and
output. A single fast route does not stand in for the others.

## Hard first-check gates

### Server input to actual DMX/network output

For every Go, Flash press, Flash release, and output-changing Playback action:

- the authoritative Playback mutation succeeds;
- the first production output frame containing the change is generated and dispatched within
  **two configured output ticks** of authenticated server receipt;
- both elapsed wall time and received/changed/output tick sequence are retained; and
- the test distinguishes engine-frame completion, protocol encoding, and the actual production
  network-send boundary.

The same two-tick rule applies to UI and OSC. A fast acknowledgement without changed output,
eventual convergence, or a direct headless render does not satisfy it.

### Playback indication

The visible bundled Playback indication updates within **50 ms**:

- for local UI, measure from the actual pointer/key input event to the corresponding pressed,
  flashed, released, current-Cue, or fader indication;
- for OSC, measure from authenticated server receipt to the corresponding bundled Playback
  indication; and
- reconcile optimistic feedback with the authoritative event without flicker, double application,
  jumping, or reversal unless the action fails.

Cue fade progress may animate locally from authoritative timing metadata. Current Cue identity,
Flash state, run/pause state, and action failure are semantic events and may not wait for a passive
telemetry sample.

### Fixture Sheet convergence and backpressure

Fixture Sheet may sample passive changing values. For every measured output change:

- a relevant visible Fixture Sheet value may take up to **500 ms** to converge;
- intermediate passive values may be skipped;
- the newest authoritative value replaces superseded work;
- no consumer accumulates an unbounded sequence of value changes;
- at most one replaceable pending latest-value update is retained per consumer and scope;
- queue depth and memory return to their steady bound after bursts; and
- Fixture Sheet projection, serialization, delivery, parsing, application, or painting does not
  delay output or Playback feedback.

Fixture Sheet remains open, scrollable, and selectable. A loading overlay, frozen scroll surface,
ever-growing queue, or eventual result produced by processing every stale sample is a failure.

### Output and API isolation

During the complete Stage-plus-Fixture-Sheet window:

- output sustains configured 60 Hz with zero deadline misses;
- running Cue fades and Dynamics remain fluent at the output boundary;
- output p99 stays within the established isolation budget compared with the same activity without
  UI consumers;
- Stage, Fixture Sheet, API projection, serialization, and socket delivery perform no awaited work
  on the engine/output path;
- stalled visualization and ordinary clients do not change output cadence or action latency; and
- non-critical updates are discarded before they create backpressure.

API routes and client calls touched by required work follow
[`docs/engineering/api-rules.md`](../../../engineering/api-rules.md).

## Decision after the mandatory first check

### If the existing application already passes

Do not perform broad architecture work in this plan.

1. Commit the permanent packaged benchmark and regular-suite registration.
2. Add `## Result` with the exact command, duration, show inventory, active surfaces, hardware,
   build profile, action distributions, output evidence, UI feedback latency, Fixture Sheet
   convergence, queue/resource bounds, and raw report paths.
3. Classify every remaining requirement in this plan as proven complete or still unproven.
4. Only after the passing evidence exists, split genuinely remaining improvements into one or
   more new pending plans immediately after this plan and before
   [Macros](32-macros-and-scheduled-macros.md). Do not copy proven work into those plans.
5. Remove that split scope from this plan, record the benchmark-backed Result, and move this plan
   to `finished/` with the semantic benchmark commit.
6. Continue with the newly split improvement plans before Macros.

This is the skip path: a passing baseline authorizes benchmark/documentation work and later
splitting only, not speculative implementation.

### If the existing application fails

Keep this complete plan in `doing/`. Its improvements remain more important than Macros or any
later queue item and must not be split or deferred.

1. Retain the failing baseline report.
2. Identify the first failed hard gate and its measured phase.
3. Implement the engine, API, client, renderer, warm-surface, telemetry, and isolation
   improvements below in coherent measured slices, starting with the first failure.
4. Rerun the same committed benchmark after each coherent slice.
5. Keep the scope here until the exact Stage-plus-Fixture-Sheet benchmark and applicable
   acceptance items pass.
6. Run focused and proportionate broader checks for each touched boundary.
7. Finish this plan before claiming Macros.

Do not weaken the two-tick output, 50 ms Playback indication, 500 ms Fixture Sheet,
zero-deadline-miss, or bounded-queue gates to obtain a pass.

## Improvements owned here if the first check fails

### Engine and publication isolation

The engine/output tick samples time-dependent state, resolves contributions, projects fixtures,
constructs universes, encodes and dispatches output, and publishes only a bounded immutable
reference or latest-frame notification.

No client projection, JSON serialization, client socket write, React state, WebView callback,
Fixture Sheet calculation, Stage work, or renderer work may execute on or be awaited by that path.

Downstream delivery:

- uses bounded overwrite-old/latest-value storage;
- retains at most one replaceable sample per consumer and scope;
- separates semantic control events from sampled telemetry;
- performs client projection and serialization outside the output owner;
- applies explicit time, payload, memory, and queue budgets; and
- isolates stalled, disconnected, malformed, and slow clients.

### Warm bundled surfaces

Keep identity, structure, settings, and stable state warm for Fixture Sheet and Fixture, Preset,
Group, Cuelist, and Dynamic Built-ins. The bundled desktop reveals a usable surface immediately
without a blank replacement, spinner-only surface, or blocking complete-data request.

Cached stale-while-revalidate content is preferred. Browser-connected cold navigation may take
approximately 500 ms. Heavy cold capabilities such as a never-opened Stage, very large Timecode
editor, asset import, or model decoding may show explicit progress.

### Efficient UI state and telemetry

Implement as required by measurements:

- capability-owned normalized warm stores;
- semantic invalidation and targeted recovery;
- visible-row and visible-column projection;
- virtualized grids with stable selection and scrolling;
- selector-level subscriptions;
- one shared decoded update per relevant desk/capability;
- bounded latest-value passive telemetry;
- local Cue-progress animation from authoritative timing events;
- operated-control ownership with coherent reconciliation; and
- priority lanes for control events, Playback state, visible operated values, passive telemetry,
  DMX inspection, and Stage.

Fixture Sheet shows stable base/programming values plus Dynamic identity and state rather than
every resolved Dynamic sample. Newly visible rows render cached content immediately and normally
refresh within 100–200 ms; the hard gate remains 500 ms. Hidden surfaces, off-screen rows, and
invisible columns do not retain high-frequency subscriptions.

### Supported capacity and independent sweeps

The product profiles remain:

- approximately 300 fixtures for recommended real-time built-in Stage;
- exactly 1,000 fixtures at 60 Hz on documented sufficiently fast hardware;
- preferably no more than 16 substantially occupied universes for the representative
  1,000-fixture show;
- 32 universes as the supported upper output stress profile; and
- 64 universes only as optional evidence.

More than 1,000 fixtures, more than 32 universes, and 2,000- or 4,000-fixture shows are
non-blocking breakpoint probes, not supported interactive capacity.

Vary one primary dimension at a time:

- fixture count;
- universe count;
- active Cuelist and simultaneous fade count;
- Dynamic instance count;
- Dynamic lanes per instance;
- targets and attributes per Dynamic; and
- active UI surfaces.

Twenty Dynamics is representative, not a maximum. Capacity probes find the practical limit
without combining every scaling axis into one uninterpretable show.

### Attributable breakpoint reports

Every retained breakpoint report names:

1. the first subsystem and workload step to exceed its budget;
2. the next two largest or fastest-growing contributors;
3. the scaling slope between steps;
4. output safety and action latency at the breaking point; and
5. whether shedding passive telemetry or Stage restores the required budget.

Instrument Dynamic sampling, contribution collection/arbitration, fixture/head evaluation,
fixture-to-DMX projection, universe construction, protocol encoding/network send, API projection,
serialization/payload, WebView delivery/parsing, client-store application, Fixture Sheet paint,
and Stage application/rendering separately where applicable.

## Acceptance coverage

1. The exact 1,000-instance show runs in the packaged bundled desktop at configured 60 Hz with one
   3D Stage and one Fixture Sheet open.
2. Representative Cuelists, Cue/Playback fades, and Dynamics remain active.
3. UI and OSC Go reach the first changed production DMX/network frame within two ticks.
4. UI and OSC Flash press and release reach corresponding changed output within two ticks.
5. Bundled Playback indication for Go, Flash, release, and operated level updates within 50 ms.
6. Relevant visible Fixture Sheet values converge within 500 ms while scrolling and selection
   remain responsive.
7. Passive bursts overwrite old work, retain bounded queues, and do not replay stale samples.
8. Production output sustains 60 Hz with zero deadline misses and no disallowed p99 regression
   while Stage and Fixture Sheet are active.
9. Stalled visualization and API consumers do not affect output, action timing, or feedback.
10. The benchmark is committed, registered in the regular performance suite, retains raw reports,
    and fails deterministically when a hard gate is exceeded.
11. Ordinary bundled Fixture, Preset, Group, Cuelist, and Dynamic surfaces reveal warm usable
    content without blocking loading state.
12. If capacity sweeps remain in this plan after a failed first check, they vary scale dimensions
    independently and report the first three limiting contributors.

## Verification and completion

Run focused tests for every touched owner, the permanent packaged benchmark, the regular
performance suite, the large end-to-end suite when production behavior changes, and the
authoritative `npm run open` workflow. Inspect readiness, bootstrap timing, output diagnostics,
and `.artifacts/runtime/light-data/light-headless.log`.

Before moving to `finished/`:

1. add a complete `## Result`;
2. identify whether the pass-first or repair path was used;
3. record the benchmark and production commits;
4. if the first check passed, create post-Macros plans containing only genuinely remaining work;
5. if it failed, complete the applicable improvements here before continuing;
6. verify regular benchmark registration from a clean invocation; and
7. move this file only after the semantic commit exists.
