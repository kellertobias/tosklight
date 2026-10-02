# Position tracking benchmarks

TL-615 measures the captured Position tracking and fit-cache path in isolation. It drives the
real code path:

1. Engine capture.
2. Hybrid static and Dynamic preparation with the Position observer.
3. Captured geometry and the tracking census.
4. Calibrated fitting.
5. The engine's native finalizer.

It does not use a direct solver loop or a fake resolver. It runs on contract-enabled *test*
Engine and Runtime instances. The production `SUPPORTED_PROGRAMMING_CONTRACT` stays `0`.

The output is gated software evidence. It is not production cadence, PSN delivery, native Stage,
operator readout, physical lamp or frame-rate acceptance. TL-596 keeps the following:

- production output deadlines;
- mixed Color/UV/Focus/Dynamics workloads;
- source and binary attribution;
- the fixed full-rig gates.

The TL-564 workload builders, the TL-604 source fingerprinting and the existing performance
gates are reused or left unchanged. None of them are duplicated.

## Files

| File | Role |
|---|---|
| `crates/light/adapters/headless/src/runtime/output_scheduler/dynamic_projection/physical_adapter/position/tests/tracking_bench.rs` | Workload, smoke, manual benchmarks and report writer |
| `docs/engineering/position-tracking-benchmarks.md` | This guide |

The module is registered as `mod tracking_bench;` in `position/tests.rs`. It reuses these
existing test helpers from the Position tests:

- `Rig`, `moving_head`, `point`, `patched`, `channel`, `target` and `angles`;
- the shared-Pan/independent-Tilt profile and its exactly encoded Origin Targets, through
  `current_cohort::shared_rig`.

## Workload

A requested number of *physical instances* is realized as follows. A physical instance is a
root or an independently placed multipatch copy.

| Class | Share | Physical instances | Position owners |
|---|---|---|---|
| Point-Target roots, each with one physical copy | rest ÷ 2 | 2 each | the root |
| Angle roots, unrelated to every Point | 10% (plus any odd remainder) | 1 each | the root |
| Shared-axis roots: shared Pan, two independent Tilt logical heads | 10% | 1 each | two logical heads |

Point-Target roots:

- Each root has a `position_master` mount Point.
- Each root aims at an aim Point through a referenced local offset. The offsets are
  `[1, 0, 0]`, `[0, 0, 0.5]` and `[-0.5, 0.5, 0]`.
- The aim Points are rotated, so each offset is rotated into the world.
- Four roots share an aim Point. Four roots share a mount Point.
- The two groupings are orthogonal. Roots that share an aim Point use different mount Points.
- Every copy has its own location.

The sizes are 300 and 1,000 requested physical instances. Any size of 10 or more can be set.

| Requested | Target roots + copies | Angle roots | Shared roots / logical heads | Aim / mount Points |
|---:|---:|---:|---:|---:|
| 20 (smoke) | 8 + 8 | 2 | 2 / 4 | 2 / 2 |
| 300 | 120 + 120 | 30 | 30 / 60 | 30 / 30 |
| 1,000 | 400 + 400 | 100 | 100 / 200 | 100 / 100 |

The report records the requested, planned and realized counts separately. The realized counts
are read back from the engine snapshot.

The following inputs are fixed and deterministic:

- All fixture, copy, logical-head and Point IDs derive from one fixed seed.
- Fixture numbers are unique.
- Every root and copy has its own valid, non-overlapping universe address: 64 eight-channel
  slots per universe.
- Motion toggles between two fixed values, so every frame of a moving scenario changes the
  moved Points.

The synthetic profile builders still assign random internal profile, channel and emitter
UUIDs. This does not change the workload's shape.

## Scenarios

Every scenario first runs unchanged frames until accepted fits converge. Convergence means
zero fits and a cache hit for every physical instance, within at most 12 frames. Then it runs
the warmup frames, which are not recorded, and then the sample frames.

| Scenario | Changes every frame | Exact functional expectation |
|---|---|---|
| `warm-unchanged` | nothing | 0 fits; every instance is a cache hit |
| `aim-one-translate` | one aim Point position | only that Point's roots and copies refit and are dirty |
| `aim-subset-translate` | about 10% of the aim Points | as above |
| `aim-all-translate` | every aim Point; the mounts stay fixed | every Target root and copy; the Angle and shared roots still reuse |
| `aim-subset-rotate` | about 10% of the aim Points rotate | dependents refit conservatively; see the observations |
| `mount-one-translate` / `mount-subset-translate` / `mount-all-translate` | mounting Point positions | only the mounted roots and copies |
| `mount-subset-rotate` | about 10% of the mounting Points rotate | as above |
| `native-edit-one` / `native-edit-subset` | an unrelated native control (`frost`) on one or about 10% of the Target roots | observed, not prescribed; Pan/Tilt must not move |
| `pending-pair-warm-unchanged` | nothing, through the paired Pending path | both branches converge to fit reuse; nothing is dirty |
| `pending-pair-aim-subset-translate` | about 10% of the aim Points, through the paired Pending path | exact changed Points and dirty instances in both branches; only dependent instances refit |

Live scenarios call `Engine::prepare_output_frame`, then
`prepare_captured_hybrid_frame_with_observer` with a `PositionFrameObserver`, then
`finalize_live_physical_frame`. This is the same sequence as the existing `prepare_live` test
helper, with test-side phase timestamps added.

Pending scenarios run the actual `PairedPendingHistory` window through
`RetainedPreloadHybridEvaluator` and `PositionPreloadObserver` over `PhysicalPreloadLanes`.
They run after every Live scenario, because arming preload is the bench's last mutation.

## Functional assertions

Functional checks run on every warmup and sample frame. Their failures are collected per
scenario. They are never mixed with the timing.

- **Source intent is kept.** For every owner, the stored programmer value and the original
  `PositionRequest::Intent` equal the value written at construction. No owner is held.
- **No invented Dynamics.** Static motion never creates a Dynamic instance.
- **Fits are correct.** Every fitted Target has an angular error below 0.05°.
- **The finalizer encodes the fitted commands.** It encodes every proposed native write.
  Every rendered instance is complete.
- **Every instance is accounted for.** For each physical instance, fits plus cache hits
  equals the instance count.
- **Exact dirty sets.** For Point motion, the accepted `TrackingSnapshot` matches the
  workload model exactly in its changed Points and its dirty `(root, destination)` set. The
  fits equal the dependent instances. Dependent rows report `geometry_dirty` and reuse
  nothing. Every unrelated row reuses one fit per destination.
- **Unrelated instances do not move.** No unrelated destination changes its Pan/Tilt native
  commands.
- **Shared-axis peers share one memo.** After each convergence, both logical heads of every
  shared-axis root carry the identical accepted fit memo `Arc`.
- **Pending stays isolated.** Every paired window succeeds. Neither branch borrows Live
  tracking. Live tracking is unchanged after Pending evaluation.
- **Pending reuse crosses frame bundles.** Both branches converge to zero fits before
  measured windows. Unchanged windows reuse every fit; moving windows refit only their
  dirty dependents. The 20-instance smoke requires 0 fits / 40 hits unchanged and
  16 fits / 24 hits for one moving aim Point, summed across the independent branches.

The smoke runs in normal test runs: 20 instances, 1 warmup and 2 samples. It also asserts the
exact counter values for that size.

## Commands

Run the commands from the repository root. Cargo writes to the canonical target directory.
Timing is never asserted.

Deterministic smoke (also part of `cargo test -p light-headless-runtime --lib`):

```sh
CARGO_TARGET_DIR=$PWD/.artifacts/build/cargo LIGHT_TMP_DIR=$PWD/.artifacts/tmp \
  cargo test -p light-headless-runtime --lib tracking_bench
```

Manual release benchmarks. Both ignored tests run serially:

```sh
node --input-type=module -e "
import fs from 'node:fs';
import { collectSourceManifest } from './tools/semantic-source-manifest.mjs';
const m = collectSourceManifest({ root: process.cwd() });
fs.writeFileSync(process.argv[1], JSON.stringify(m, null, 1));
console.log(m.sourceSha256);
" .artifacts/tmp/position-bench-source-manifest.json

LIGHT_POSITION_BENCH_SOURCE_SHA256=<printed sha256> \
LIGHT_POSITION_BENCH_SOURCE_MANIFEST=$PWD/.artifacts/tmp/position-bench-source-manifest.json \
LIGHT_POSITION_BENCH_HOST_CPU="$(sysctl -n machdep.cpu.brand_string)" \
CARGO_TARGET_DIR=$PWD/.artifacts/build/cargo LIGHT_TMP_DIR=$PWD/.artifacts/tmp \
  cargo test --release -p light-headless-runtime --lib tracking_bench -- --ignored --nocapture --test-threads=1
```

To run one size only, add its test name as the filter:
`manual_position_tracking_benchmark_300_instances` or
`manual_position_tracking_benchmark_1000_instances`.

The following optional overrides are available:

- `LIGHT_POSITION_BENCH_INSTANCES` overrides the size.
- `LIGHT_POSITION_BENCH_WARMUP` sets the warmup frames. The default is 5 and the maximum is
  100.
- `LIGHT_POSITION_BENCH_SAMPLES` sets the sample frames. The default is 30 and the range is
  1–1,000.

When no source manifest is supplied, the source identity is reported as `unavailable`. The
benchmark records the supplied digest but does not verify it.

## Output

Reports are written to the first of these that is set:

1. `$LIGHT_PERFORMANCE_DIR`
2. `$LIGHT_ARTIFACTS_DIR/performance`
3. `<repo>/.artifacts/performance`

A relative override resolves against the repository root. Each run creates a new directory,
`position-tracking/<UTC timestamp>-n<size>-pid<pid>/`. The run uses `create_dir` and
`create_new`, so an existing run directory or file is never overwritten.

- `report.json` uses the schema `tosklight.position-tracking-benchmark/v1` and contains:
  - `claims`, with every acceptance claim set to `false`;
  - `run`, including the wall elapsed time and the paths exercised;
  - `parameters`: the seed, warmup, samples and the convergence bound;
  - `time`: simulated `ManualClock` milliseconds per frame. Observed output and PSN rates are
    `unavailable`;
  - `build`: whether debug assertions were on, the test binary path, SHA-256 and size, the
    supplied source identity and the test contract version;
  - `host`;
  - `workload`: requested, planned and realized counts;
  - per-scenario timing and counter distributions, raw per-frame samples and functional
    results.
- `summary.md` holds the headline table.

Timing uses monotonic `std::time::Instant` around test-side phase boundaries:

- Live:
  - `capture` covers `Engine::prepare_output_frame`.
  - `prepare` covers static/Dynamic preparation, geometry, the tracking census and fitting.
  - `finalize` covers native projection and encoding, the engine render and lane acceptance.
- Pending:
  - `captureAndRetain`.
  - `pairedEvaluation`.

Geometry-only, fitting-only, native-encoding-only and per-branch times have no existing API.
They stay `unavailable` rather than adding production instrumentation.

Distributions use nearest-rank p50/p95/p99. With 30 samples, p99 equals the maximum.

Counters come from existing APIs:

- `PositionAdapter::counters`: fits, fit-cache hits, candidate evaluations and descriptor
  compiles, as a delta per frame;
- the accepted `TrackingSnapshot`: changed Points and dirty instances;
- `PositionQuality`: reused fits and `geometry_dirty` rows;
- finalized `native_raw` compared with the previous frame.

A missing metric is reported as `{"status": "unavailable", "reason": …}`, never as `0`.

## Observations

These are behaviors the benchmark reports. They are not requirements.

- **Rotation dirtying is conservative.** Rotating an aim Point dirties and refits every
  dependent instance. This includes a root whose local offset lies on the rotation axis. Its
  refit reproduces the same Pan/Tilt commands.
- **Native memo misses are conservative.** An unrelated native control edit makes the edited
  root and copy miss the fit memo, because the full native baseline is part of the key. The
  refit is cheap: it is warm-started from the accepted joints. Pan/Tilt never move, and all
  other owners still reuse their fits. The cache key is not weakened.
- **Original TL-615 reports recorded no Pending reuse.** Those immutable reports remain
  evidence of the source they measured. TL-632 identified the cause: the memo compared exact
  `CapturedFrameLane` equality, including a new bundle identity and advancing render revision
  on every paired window. This guaranteed a miss despite unchanged solver inputs.
  The repair compares a separately named stable memo domain: Live, or the Preload state
  identity plus Release branch. The state belongs to one retained episode. Exact frame-token
  admission remains unchanged, and all generation, compatibility, cohort, solver-input,
  full native-baseline and dirty-geometry guards still apply. Re-run the smoke and any manual
  measurement on the repaired source; this documentation update provides no new timing result.

## Limitations

- These are microbenchmarks. Frames run back to back on one thread, other host processes are
  not controlled, and the CPU model is only supplied, not detected.
- The profiles are synthetic U16 Pan/Tilt profiles. They describe authored commands, not
  measured motor feedback.
- The tests do not open a PSN socket, render a native Stage or run an output loop.
- Live timing includes the test-side `origins.clone()` and the
  `DynamicOutputFrameScratch::default()` allocation of the `prepare_live` path. It excludes
  workload mutation and functional checks.
