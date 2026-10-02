# Color fitting benchmarks

TL-624 measures the work the captured Color/UV physical adapter does when it compiles and
fits. It drives the real code path for one patched fixture per capture:

1. Engine capture (`Engine::prepare_output_frame`) and its frame token.
2. The scalar static-family frame.
3. The captured static geometry.
4. `ColorAdapter::compile`, which compiles the destination descriptor and fitter.
5. `ColorAdapter::resolve` against the token-bound native raw values.
6. Complete owned writes (`validate_complete_writes`).
7. Encoded forward checks, run outside the timing.

It does not use a direct solver loop or a fake resolver. The production
`SUPPORTED_PROGRAMMING_CONTRACT` stays `0`, and no production code is instrumented.

The output is a synthetic, sequential fit workload. It is not any of the following:

- a simultaneous output frame;
- a complete render or output deadline;
- a physical lamp colour match;
- native Stage or browser Stage evidence;
- frame-rate acceptance.

TL-596 keeps the end-to-end output deadlines, paired builds, dirty-work gates and fixed
full-rig gates. The benchmark sets no thresholds.

## Files

| File | Role |
|---|---|
| `crates/light/adapters/headless/src/runtime/output_scheduler/dynamic_projection/physical_adapter/color/tests_fitting_bench.rs` | Workload, smoke, manual benchmark and report writer |
| `docs/engineering/color-fitting-benchmarks.md` | This guide |

The module is registered as a `#[cfg(test)] mod tests_fitting_bench;` in `color.rs`. It reuses
these existing test helpers without widening their visibility:

- `tests_direct::DirectRig`, which retains one compiled descriptor per installed generation;
- `tests_direct::Resolved` and `Resolved::assert_forward`;
- `tests::{intent, magenta, warm_white, program}`;
- the `profiles` builders `rgb`, `rgbwauv`, `cmy_wheel`, `hybrid`, `wheel_only`, `Builder`,
  `spectrum` and `wheel_slots`.

Three helpers are private to their own test modules, so they are copied rather than widened:

- the DMX `decode` helper and the encoded check of `tests::Resolved::verify`;
- the report helpers of the Position benchmark (`position/tests/tracking_bench.rs`).

## Workload

Seven fixture capabilities:

| Fixture | Capability | UV model |
|---|---|---|
| `additive-rgb` | additive RGB, U16 red | no UV emitter |
| `additive-rgbwauv-known-leakage` | additive RGBWA plus UV | known UV leakage (0.02, 0.005, 0.1) |
| `additive-rgbwauv-unknown-leakage` | additive RGBWA plus UV | unknown UV leakage |
| `subtractive-cmy-wheel` | CMY flags plus a colour wheel | no UV emitter |
| `hybrid-rgbw-wheel` | `profiles::hybrid`: XYZ-only additive RGBW behind a spectral wheel | no UV emitter |
| `hybrid-spectral-rgbw-wheel` | the same topology with spectral emitters, built in the bench file | no UV emitter |
| `wheel-only` | fixed source through one wheel with a rotation range | no UV emitter |

Seven request scenarios. Each scenario is run against every fixture:

| Scenario | Request | Changes every frame |
|---|---|---|
| `warm-unchanged` | TL-557 warm white | nothing: one unchanged value object |
| `white-blend-sweep` | warm white | White Blend 0, 0.25, 0.5, 0.75, 1 |
| `cct-sweep` | warm white | CCT 2700, 3200, 4300, 5600, 6500 K |
| `duv-sweep` | warm white | Duv −0.006 to 0.006 |
| `uv-sweep` | TL-557 magenta | UV 0, 0.25, 0.5, 0.75, 1 |
| `hue-candidate-sweep` | red, green, blue, magenta, cyan, yellow, white | the recipe, so wheel and flag candidates re-rank |
| `wheel-constraint-sweep` | blue | no constraint, own red slot, own blue slot, or a foreign wheel's constraint |

The wheel-constraint scenario does not apply to the three wheel-less fixtures. Those runs are
reported as `not-applicable`, which leaves 46 measured runs.

The request order is fixed. It comes from xorshift64* with seed `0x7162400000000001`, and a
sweep never repeats the same request twice in a row. The profile builders still assign random
internal profile, channel and fixture UUIDs. This does not change the workload's shape. The
report includes a SHA-256 digest of the canonical workload description.

Every capture holds exactly one fixture. A 1,000-head frame is never assembled from sequential
captures, and the report records `simultaneousOutputFrame: false`. Large multi-fixture modes
are not implemented. They are optional, and they cannot replace the TL-596 frame gates.

## Cold and warm

Each run uses a new `DirectRig`.

- **Cold sample.** `DirectRig::install` creates a new fixture list and generation. The first
  `DirectRig::resolve` then compiles the descriptor and the fitter, and resolves. The same
  captured snapshot is also compiled on a fresh `ColorAdapter`. That gives the `isolatedCompile`
  time, and its footprint must equal the retained descriptor's footprint.
- **Warm frame.** The descriptor that `DirectRig` retained for the last cold generation is
  resolved again with the same phases as `DirectRig::resolve`, plus timestamps. The previous
  frame's continuity is passed in, as the lane passes it.

Because warm frames reuse the unchanged descriptor, compiling is never counted as fitting. The
run ends with one more `DirectRig::resolve`. It must return the same descriptor `Rc` and must not
compile.

`DirectRig`'s `ManualClock` is private, so warm frames do not advance it. Static Color fitting
reads no time. Each frame still has its own capture identity and token.

## Functional assertions

These checks run on every cold sample and every warm or sample frame. They are collected per
run and never mixed with the timing.

- **Writes are complete and owned.** `validate_complete_writes` passes. Every write is on the
  target and is one of the head's own Color controls, and each control is written exactly once.
- **The request is kept.** The published request equals the authored `ColorIntent`, and the
  value object is unchanged.
- **Forward equality.** The scalar baseline with the writes applied is encoded to DMX and
  decoded again. Re-running the compiled forward model must reproduce the published known XYZ,
  the complete visible output and the UV drive. `Resolved::assert_forward` must also pass.
- **Generation and token.** Every warm capture belongs to the same generation, meaning the
  same fixture-list `Arc`. No two frames share a capture token.
- **No warm compile.** Each warm frame has 0 descriptor compiles, 0 fitter compiles and 0
  fitter-cache lookups, and exactly 1 resolve.
- **Counters match the published work.** The per-frame adapter counter deltas for fits,
  candidates and level solves equal the resolution's published `ColorSolveWork`.
- **Unchanged means identical.** In `warm-unchanged`, every frame has identical writes and
  identical counters.

The deterministic smoke runs 2 cold samples, 1 warmup frame and 6 sample frames per run. It
also asserts these facts:

- Every cold sample compiles one descriptor and one fitter, and the isolated compile does the
  same. This keeps compiling bounded and stable.
- Every warm frame compiles nothing, makes one resolve and makes at least one fit and one
  forward evaluation.
- Candidate work per capability is exact. In the order candidates, visible solves, level
  solves:
  - additive heads: 1, 1, 1;
  - CMY plus wheel: 81, 0, 0;
  - wheel only: 3, 0, 0;
  - spectral hybrid: 3, 3, 3.
- The XYZ-only `profiles::hybrid` is always `UnknownAppearance`, with 0 visible solves. It is
  never counted as a numerical fit.
- **UV is classified separately from numerical matching:**
  - With known leakage, every UV request is a `numerical-fit`. A nonzero UV runs 1 fixed-offset
    solve and 43 level solves; UV 0 runs none and 1.
  - With unknown leakage, a nonzero UV is `unknown-uv-leakage` (`PredictionIncomplete`) and
    never runs the fixed-offset search.
  - On RGB, CMY and hybrid fixtures, a nonzero UV is `uv-unsupported`.
- White Blend, CCT and Duv requests are numerical fits with level solves on both RGBWAUV heads
  and on the spectral hybrid.
- An own wheel constraint is `Applied` and narrows the candidates: to 1 on wheels and hybrids,
  and to 27 on CMY plus wheel. A foreign constraint is `SourceMismatch`.

## Commands

Run the commands from the repository root. Timing is never asserted.

Deterministic smoke (also part of `cargo test -p light-headless-runtime --lib`):

```sh
CARGO_TARGET_DIR=$PWD/.artifacts/build/cargo LIGHT_TMP_DIR=$PWD/.artifacts/tmp \
  cargo test -p light-headless-runtime --lib tests_fitting_bench
```

Manual release benchmark, with a TL-604 source identity:

```sh
node --input-type=module -e "
import fs from 'node:fs';
import { collectSourceManifest } from './tools/semantic-source-manifest.mjs';
const m = collectSourceManifest({ root: process.cwd() });
fs.writeFileSync(process.argv[1], JSON.stringify(m, null, 1));
console.log(m.sourceSha256);
" .artifacts/tmp/color-bench-source-manifest.json

LIGHT_COLOR_BENCH_SOURCE_SHA256=<printed sha256> \
LIGHT_COLOR_BENCH_SOURCE_MANIFEST=$PWD/.artifacts/tmp/color-bench-source-manifest.json \
LIGHT_COLOR_BENCH_SOURCE_ORIGIN=tools/semantic-source-manifest.mjs \
LIGHT_COLOR_BENCH_HOST_CPU="$(sysctl -n machdep.cpu.brand_string)" \
LIGHT_COLOR_BENCH_RUSTC="$(rustc -V)" \
CARGO_TARGET_DIR=$PWD/.artifacts/build/cargo LIGHT_TMP_DIR=$PWD/.artifacts/tmp \
  cargo test --release -p light-headless-runtime --lib manual_color_fitting_benchmark \
  -- --ignored --nocapture --test-threads=1
```

These optional overrides are available:

| Variable | Default | Range |
|---|---:|---|
| `LIGHT_COLOR_BENCH_COLD` (cold samples per run) | 20 | 1–200 |
| `LIGHT_COLOR_BENCH_WARMUP` (warmup frames per run) | 20 | 0–1,000 |
| `LIGHT_COLOR_BENCH_SAMPLES` (sample frames per run) | 300 | 1–10,000 |

When no source digest is supplied, the source identity is reported as `unavailable`. The
benchmark records a supplied digest but does not verify it.

## Output

Reports are written to the first of these that is set:

1. `$LIGHT_PERFORMANCE_DIR`
2. `$LIGHT_ARTIFACTS_DIR/performance`
3. `<repo>/.artifacts/performance`

Each run creates a new directory, `color-fitting/<UTC timestamp>-s<samples>-pid<pid>/`. The
benchmark uses `create_dir` and `create_new`, so an existing run directory or file is never
overwritten.

`report.json` uses the schema `tosklight.color-fitting-benchmark/v1` and contains:

- `claims`: every acceptance claim is `false`;
- `run`: the paths exercised, the output directory, the rerun command and the parameters;
- `build`: the profile, the test binary's path, SHA-256 and size, the supplied source identity
  and the supplied rustc version;
- `host`;
- `workload`: the identity description and its SHA-256;
- for every fixture × scenario run:
  - cold distributions of `install`, `firstResolve` and `isolatedCompile`, with counters;
  - warm distributions of end-to-end time and of the `capture`, `scalar`, `geometry`, `resolve`
    and `validate` phases, plus end-to-end time per result class;
  - per-frame counter distributions;
  - histograms of class, colour match (numerical fits only), visible status, UV status and
    constraint status;
  - per-request breakdowns, raw frames and the functional result.

`summary.md` holds the headline table.

Distributions report `total`, `min`, nearest-rank `p50`/`p95`/`p99`, `max` and `mean`.

Counters come from `ColorAdapter::counters`, as a delta per frame:

- resolves, fits and refits;
- candidates ranked, visible solves, fixed-offset solves, level solves and forward evaluations;
- descriptor compiles, fitter compiles, fitter-cache hits and failures;
- shared conflicts.

These metrics have no existing API. They are reported as
`{"status": "unavailable", "reason": …}` and never as `0`:

- fitter-only time;
- per-candidate time;
- native encoding time, because the engine finalizer is not on this path.

`fitResultCache` is reported as `not-present`: the Color adapter keeps no fit-result memo, so
every resolve fits.

## Observations

These are behaviors the benchmark reports. They are not requirements.

- **Fixed-UV leakage dominates additive fitting cost.** With known leakage, a nonzero UV request
  runs the bounded fixed-offset search: 43 level solves instead of 1. In the first release run,
  that took a warm resolve of about 38–41 µs, against about 1.7 µs for UV 0 on the same head.
- **Unknown leakage costs no extra work, but it is not a match.** Unknown leakage does no extra
  numerical work, and its frames are reported as `unknown-uv-leakage`.
- **Unsupported UV is reported, not substituted.** It stays `uv-unsupported` and never produces
  violet.
- **The stock hybrid profile has no known appearance.** `profiles::hybrid` has XYZ-only
  emitters behind a spectral wheel, so no candidate has a known appearance and its visible
  controls are retained. Its timings measure the unknown-appearance path, not numerical
  hybrid fitting. The spectral hybrid variant measures numerical hybrid fitting.
- **Cold cost is almost all descriptor and fitter compilation.** `isolatedCompile` is close to
  `firstResolve` on every fixture. For example, CMY plus wheel compiles in about 2.3 ms, and
  its warm frames take about 7.5 µs.

## Limitations

- These are microbenchmarks. They run one fixture per capture, frames back to back on one
  thread, and other host processes are not controlled. The CPU model is supplied, not detected.
- The profiles are synthetic test profiles. They describe modeled emitters and filters, not
  measured lamps.
- Lane acceptance, the hybrid observer, the engine finalizer, Dynamic or Direct programs,
  multipatch copies and multi-head targets are not exercised by this benchmark. Existing tests
  cover them functionally.
