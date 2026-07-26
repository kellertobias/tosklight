# Sustained Complex-show Output Benchmark

## Goal

Provide a reproducible one- or two-minute release benchmark that runs a complex, fully packed show,
reports its average and minimum frame rate, and fails if output ever falls below the required
minimum.

Estimated effort: 0.5 Codex day.

## Queue dependency

Claimed after plans 09 and 10 established the release benchmark and shared control-surface
contracts. The benchmark is an engine/output acceptance case; it does not require physical control
hardware.

## Required work

1. Build a deterministic complex show from the shipped fixture packages: 20 Sunstrip LED RGB,
   40 Robin 600X LEDWash, 32 Robin DLS Profile, 32 Robin LEDBeam 150, and enough ordinary RGB PARs
   to fill 32 universes, with overlapping Group, Cue/Playback, animated phaser, and Programmer
   contributions.
2. Include that show work in every timed frame before production Art-Net and sACN encoding and
   loopback delivery; retain external sampled replacement batches as a separately labeled
   diagnostic rather than presenting them as part of the production scheduled pipeline.
3. Support 60- and 120-second release trials without shortening the measured path, scheduling
   modest headroom above the required minimum so the run proves at least 100 completed frames in
   every second rather than merely targeting the boundary.
4. Report average completed frame rate, minimum completed frame rate across every one-second
   interval, dropped frames, deferred frames, and deadline misses.
5. Fail the hard-floor run unless the average and every measured one-second window sustain at least
   100 Hz; retain dropped, deferred, and late-frame counters as diagnostics.
6. Provide one repository command that runs the sustained case, retains raw JSON and stderr under
   `.artifacts/performance`, and prints a compact operator summary.

## Acceptance and verification

- The timed scenario proves every universe remains fully patched and encoded on every completed
  frame.
- The report identifies the show workload and distinguishes built-in timed phaser sampling from the
  separate external sampled-replacement diagnostic.
- Average and minimum one-second frame rates are explicit numeric fields rather than inferred from
  latency percentiles.
- The hard-floor gate fails when average frame rate or any one-second window falls below 100 Hz;
  dropped, deferred, and deadline-missed frames remain visible for diagnosis.
- Focused unit tests cover aggregation and gate failures; a real 60- or 120-second release trial
  retains a passing or failing raw report.

## Result

### Changes

- Extended the release benchmark schema with explicit average, wall-clock average, minimum
  one-second frame rate, below-floor window count, and gate result.
- Added an optional scheduled-rate override while retaining each named profile's independent
  minimum. The sustained command schedules 125 Hz and requires at least 100 completed frames in
  every measured second.
- Added `npm run benchmark:sustained-output`, which runs 60- or 120-second release trials, retains
  JSON and stderr under `.artifacts/performance`, and prints a compact pass/fail summary.
- The deterministic show loads the shipped packages and patches 20 Sunstrip LED RGB, 40 Robin 600X
  LEDWash, 32 Robin DLS Profile, and 32 Robin LEDBeam 150 fixtures. They use 4,288 slots. Universe
  boundary-safe fill adds 4,000 three-channel and 24 four-channel Generic RGB LED fixtures for the
  remaining 12,096 slots: 4,148 fixtures and exactly 16,384 slots across 32 universes.
- Every timed frame combines Group, Cue/Playback, animated phaser, and Programmer contributions
  before Art-Net and sACN encoding plus benchmark-owned UDP loopback delivery. External sampled
  replacement batches remain an explicitly separate diagnostic.
- Added focused Rust aggregation/gate tests, Node report-summary tests, and the Node test to the
  architecture gate.

### Verification

- `cargo test -p light-headless --bin light-benchmark --no-default-features` — 18 passed, including
  exact shipped-profile/mode inventory and universe-fill coverage.
- `cargo run -p light-fixture --bin fixture-package -- validate ...` for the five consumed shipped
  packages — all passed.
- `node --test tools/run-sustained-output-benchmark.test.mjs` — 2 passed.
- `npx biome check tools/run-sustained-output-benchmark.mjs
  tools/run-sustained-output-benchmark.test.mjs` — passed.
- `cargo clippy -p light-headless --bin light-benchmark --no-default-features --no-deps --
  -D warnings` — passed.
- `npm run test:architecture` — passed.
- `npm run benchmark:sustained-output -- --seconds 60` — passed on Apple M5 Max in release mode:
  124.65 Hz average, 122 Hz minimum across 60 one-second windows, zero windows below the 100 Hz
  minimum. The report retained 7,479 complete frames, 478,656 encoded packets, and 489,152 received
  loopback datagrams.
- `git diff --check` for the Plan 11 paths — passed.

### Limitations

- The retained loopback timing uses the benchmark-owned UDP adapter, not production
  `NetworkOutput`; CPU utilization, allocation rate, and sound-to-light analysis remain explicitly
  unmeasured or accounted exclusions in the JSON.
- The passing run recorded 21 dropped scheduled 125 Hz ticks, 367 deferred ticks, and 369
  125 Hz-deadline misses, but its worst one-second output remained 122 Hz and therefore never fell
  below the required 100 Hz minimum.
- The earlier intentionally overloaded 2,400-sampled-replacement trial is retained locally as
  `.artifacts/performance/sustained-output-2026-07-26T17-22-25-452Z.json`. The first full-minute
  real-show trial, which identified one 91 Hz outlier window at a 110 Hz schedule, is
  `.artifacts/performance/sustained-output-2026-07-26T17-44-59-456Z.json`. The passing 125 Hz
  headroom evidence is `.artifacts/performance/sustained-output-2026-07-26T17-46-51-797Z.json`.
  Performance artifacts are ignored and are not committed.
- Focused Clippy reached unrelated concurrent changes in `crates/light/src/event/model.rs` and
  `crates/light/src/programming/service/values.rs`; it reported `large_enum_variant` and
  `nonminimal_bool`. Plan 11 does not modify those files.
- Physical controls, a Raspberry Pi, and a separate reference Mac are not requirements for this
  sustained engine/output benchmark.

### Commit

- `feat(performance): retain sustained complex-show frame rate`
