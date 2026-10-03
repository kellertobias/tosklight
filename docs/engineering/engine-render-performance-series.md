# Engine render performance series, September 2026

Where the output tick's time went before this series, what changed, and the measured result.
The commits run from `ebd7a67f9` to `565167ce6` on `main`; the base for every comparison is
`ebbaf0a64`, the commit before the first of them.

## How it was measured

Absolute tick times on a developer machine drift with whatever else is running, so the numbers
that decided each step came from alternating runs of two binaries on one machine: the previous
commit and the candidate, three rounds each, medians reported. Two runs taken minutes apart were
not trusted against each other; one step that looked like a full millisecond that way measured
neutral when alternated and was reverted.

The profiles are the repository's own benchmark binary:

```sh
# 2,000 or 4,000 mixed fixtures, 20 Dynamics, 60 Hz, encode-only
light-benchmark --protocol both --transport encode-only --seconds 8 --warmup-seconds 1 \
  --headless-stress-fixtures 2000 --fixture-package-dir assets/fixture-library

# the release gate: 4,148 fixtures with cue content at 125 Hz
light-benchmark --profile hard-floor --protocol both --transport encode-only --seconds 12 \
  --warmup-seconds 1 --rate-hz 125 --sustained-show --fixture-package-dir assets/fixture-library
```

`LIGHT_RENDER_PHASES=1` adds a per-phase breakdown to the report, and `sample(1)` on the
running process gave the function-level picture that pointed at each step.

## Result

Base `ebbaf0a64` against `565167ce6`, alternating, three rounds each, medians:

| Profile | Metric | Before | After |
| --- | --- | ---: | ---: |
| Stress, 2,000 fixtures, 60 Hz | tick p50 | 7.64 ms | 5.44 ms |
| | tick p99 | 8.22 ms | 5.97 ms |
| Stress, 4,000 fixtures, 60 Hz | tick p50 | 14.46 ms | 10.38 ms |
| | tick p99 | 15.59 ms | 12.35 ms |
| | deadline misses per run | 0 / 120 / 6 | 2 / 5 / 0 |
| Sustained show, 4,148 fixtures, 125 Hz | tick p50 | 3.82 ms | 3.44 ms |
| | tick p99 | 4.98 ms | 4.23 ms |

Every run held its configured rate before and after. At 4,000 fixtures the 60 Hz budget is
16.7 ms; p99 moved from one millisecond under it to four.

## What the profile said, and what each step did

The first reading of the phase breakdown blamed the coloured-head slow path of fixture
projection. A CPU sample showed the opposite: every fixture in the stress profile takes the fast
path, and the fast path itself was the cost. Three things dominated, all decidable before the
tick starts.

1. **String compares in the per-channel loop.** Resolving a channel compared attribute keys by
   content for every function of every channel of every head. The resolution plan now carries
   those answers, decided when the mode compiles (`crates/shared/fixture/src/profile/resolution_plan.rs`).
2. **One unnumbered value switched the whole show to name lookups.** A value the patch could not
   number lands in an overflow map; the check for it was show-wide, so every head fell back to
   hashing attribute names. The check is per fixture now, and the manufacturer's channel name is
   numbered with the patch (`crates/light/domain/engine/src/profile_value_index.rs`,
   `frame_slots.rs`).
3. **Every producer offered values by name, every tick.** Dynamics samples, cue contributions,
   Programmer values and Group programming were each looked up by attribute name on every tick,
   though the pairs they name change only when the show or the operator's edits do. The engine
   now hands out `FrameAddress`es for a patch generation (`light_core::FrameAddress`), and each
   producer remembers them: a Dynamics instance per target and lane, a compiled cue list per
   attribute, the Programmer memo per shared value vector, and Groups as a per-generation plan of
   slots. An address from another generation is ignored and the name is used, so a producer that
   has not seen a repatch is still correct.

Smaller steps: shared intensity and colour keys instead of two allocations per head per tick;
the Dynamics batch keyed with the cheap hasher; the automatic-cue transition source built for one
playback instead of all of them; the playback write lock downgraded to a read lock once the tick
has run.

## What was tried and not kept

Pooling the overflow maps with the frame storage looked like a millisecond in two runs minutes
apart and measured neutral at 2,000 fixtures and two percent slower at 4,000 when alternated. It
was reverted with those numbers in the message. The lesson is the method above.

## What remains

- Values that arrive without a slot still go by name; that is the correct case for undeclared
  attributes and is not worth more work.
- The visualizer's redraw gate no longer advances on unchanged packets, and fixture labels are
  cached in both the window and the embedded pane. The unlit-view composite skip, per-quality
  MSAA and half-resolution volumetrics from `visualizer-gpu-cost.md` remain open.
- `stage-performance-baseline.md` still points at a Playwright spec that was deleted; the
  packaged runner (`npm run benchmark:supported-scale`) is the remaining Stage gate.

## Semantic output, October 2026 (TL-596)

TL-553 below follows up on every workload this section handed over, with new numbers.

The question was whether the production semantic output path holds the established deadlines,
and whether its dirty work stays bounded. The established gates do not hold, and the
semantic capacity tiers miss by an order of magnitude. The 112-fixture TL-564 mix holds 44, 60
and 125 Hz apart from two single late frames at 125 Hz. Tracking dirties exactly the predicted
dependents. Unchanged Position targets reuse their solves, and readout consumers add no solves.
Unchanged Colour targets are re-fitted every frame.

### How it was measured

- **Path.** Every semantic number comes from `light-benchmark` through
  `light_headless_runtime::output_benchmark::LiveOutputBench`. That is the scheduler's
  `dynamic_output_frame` boundary with the production all-family opt-in. It covers Programmer
  `DynamicOn` reconciliation, Dynamics sampling, Position/Colour/Focus-Zoom solves, the final
  engine render, lane acceptance and visualization publication, plus route encoding.
- **Left out.** The ordered Playback unit of work, timecode, Hold/raw overrides and network
  send are omitted. `semantic-performance-workloads.md` describes the seam and every option.
- **Complete cost.** "Pipeline" below is capture + transaction + publication + encode. It is
  not `RenderTotal`, which excludes the capture.
- **Builds.**
  - Baseline: `f67c84e48` (`origin/main`, pre-semantic), source `85f4401a…`, binary `8a95e4a0…`.
    It was built from a detached snapshot with its own target directory and an empty frontend
    directory.
  - Candidate: `64a2ea132` plus the TL-596 working tree, source `4efaec30…` captured at build
    time, binary `06ef7036…`.
  - Both builds: `--release --locked --no-default-features`.
- **Host.** Apple M5 Max (18 logical CPUs, 64 GB), macOS 26.6. Other agents' Playwright and
  Cargo work shared the host. Every comparison alternates binaries round by round, and medians
  are reported.
- **Evidence.**
  - Raw runs and gate tables:
    `.artifacts/performance/semantic-output/tl596-final-20261003T*/{legacy,semantic,fix-paired}`.
  - 34 measured TL-564 reports per pass sit next to the inputs in
    `.artifacts/performance/semantic-workloads/491b33a7-…/cf09c8c854bb7eca/`. Their claims are
    `incomplete` (see Unavailable), and `acceptance.granted` is false.

### Gates

| Gate (existing threshold) | Result |
| --- | --- |
| Paired p99 regression ≤ max(1 ms, 5 %), stress 2,000 / 60 Hz | **fail**: 6.08 → 7.28 ms (+1.20). An earlier five-round pass measured +0.92. Borderline. |
| Paired p99 regression, stress 4,000 / 60 Hz | **fail**: 12.25 → 14.38 ms (+2.13) |
| Paired p99 regression, hard floor 4,148 / 125 Hz | pass: 5.08 → 5.86 ms (+0.78) |
| Hard floor holds 125 Hz every round (candidate) | pass, 5 of 5 |
| Typed-lane stress 2,000 / 60 Hz: rate held, 0 misses | **fail**: 7.7 Hz average, p99 133.6 ms |
| Typed-lane stress 4,000 / 60 Hz | **fail**: 3.5 Hz average, p99 296.0 ms |
| Typed-lane hard floor 4,148 / 125 Hz | **fail**: 16.3 Hz average, p99 65.2 ms |
| TL-564 mix, 27 configurations (output 44/60/125 × tracking 30/60/120 × 3 dirty scenarios): rate held, 0 misses | 25 pass. **2 fail** (all-points-move at 125 Hz with 60 and 120 Hz tracking): one miss each. Pipeline max was 6.6–6.7 ms against the 8 ms period, and the misses were late-starting frames. |
| No generation change or compile caused by motion (all semantic runs) | pass: 0 / 0 |
| `static-points`: 0 Position fits (all memo hits), static bases | pass: 0 fits, 54 hits per frame |
| `small-subset` / `all-points-move`: fits and dirty set equal the manifest's dependents | pass: 6 / 6 and 24 / 24 |
| Unchanged Colour targets skip solves | **fail**: 66 of 66 heads re-fitted every frame |
| Readout consumers (4, and 4 holding each frame 50 ms) do not multiply physical solves | pass: per-frame solves identical |
| Bounded caches and workspace | no threshold exists. Resident memory is stable across rounds but doubles with typed lanes (stress 2,000: 275 → 620 MB; 4,000: 525 → 1,128 MB; hard floor: 243 → 563 MB) |

Not run or unavailable:

- 125 Hz through the production scheduler, which clamps to 40–60 Hz. 125 Hz exists only in
  `light-benchmark`, so the 125 Hz rows above use benchmark pacing.
- The 300- and 1,000-instance Position tiers (TL-615's own benchmark), and the 970-record
  supported-scale Stage rig.
- A PSN socket. Tracking is injected with `Engine::set_tracking_frame` at output ticks.
- Native Stage and physical lamp rates.
- `transformAimP99Ms`: there is no separate phase.
- `portableShowWritesFromMotion`: the seam holds no ShowStore. Tracking only calls
  `set_tracking_frame`, and the snapshot is never replaced.
- Allocation counts: the binary forbids `unsafe`, so no counting allocator is possible.
- Send errors: the semantic runs are encode-only.

### Results

Legacy scalar workloads, five alternated rounds, medians (pipeline ms):

| Profile | Baseline p50 / p99 | Candidate p50 / p99 |
| --- | ---: | ---: |
| Stress 2,000, 60 Hz | 5.45 / 6.08 | 6.13 / 7.28 |
| Stress 4,000, 60 Hz | 11.17 / 12.25 | 13.29 / 14.38 |
| Hard floor 4,148, 125 Hz | 4.46 / 5.08 | 4.87 / 5.86 |

The legacy workload is not semantic. It offers scalar `color.*`, `pan` and `tilt` to a
contract-1 engine. A paired CPU sample of stress 2,000 put the added time in engine-wide code:

- `HeadOverlayState::resolve`;
- `EngineContributionResolver::offer_overflow` (legacy names now land in the overflow map);
- `PhysicalProjectionIndex::evaluate`;
- `profile_visual_color`;
- encoding.

The added time is not in the Position, Colour or Focus/Zoom adapters.

Typed-lane capacity variants: the same fixtures, Dynamic count and lanes per Dynamic.
Three rounds, medians, ms:

| Profile | Scalar control p99 | Typed p50 / p99 | Transaction p99 | Capture p99 | Publication p99 | Position fits / Colour fits per frame |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| Stress 2,000, 60 Hz | 6.96 | 130.4 / 133.6 | 132.8 | 0.34 | 0.52 | 995 / 2,520 |
| Stress 4,000, 60 Hz | 14.76 | 286.4 / 296.0 | 294.2 | 0.58 | 3.83 | 2,000 / 5,040 |
| Hard floor 4,148, 125 Hz | 5.76 | 61.4 / 65.2 | 63.9 | 0.74 | 0.42 | 53 of 104 / 4,408 |

TL-564 mix (112 fixtures, 20 Dynamics, 54 Position, 66 Colour and 36 optics owners):

- **Pipeline p99.** 6.6–7.0 ms at 44 Hz, 5.5–6.6 ms at 60 Hz and 4.4–4.8 ms at 125 Hz, in every
  tracking configuration.
- **Tracking-to-output p95.** About 36 ms, 21 ms and 13 ms at 30, 60 and 120 Hz tracking. The
  age of the newest sample dominates.
- **Static bases only** (Dynamics stopped):
  - 3.3 ms p50.
  - `static-points`: 0 Position fits.
  - `small-subset`: exactly the 6 dependents refit, and 48 reuse.
  - `all-points-move`: exactly 24 dependents.
- **With Dynamics running**, every animated Position owner refits each frame. The tracking
  census then marks 46 instances dirty for one moving Point, against 6 static dependents. That
  costs no extra solve here, but it is conservative.

### Local fixes (measured, with tests)

Each fix sits in the semantic output path, is behaviour-preserving and is covered by the
existing tests plus a new one. The comparison alternates pre-fix and final builds, three
rounds, and reports median transaction p50:

- **Shared-control verification.** `PhysicalAdapterLane::verify` compared every staged write
  with every earlier write. That is quadratic, and took 12 % of a typed stress-2,000 frame. It
  is one pass with a reused map now (test:
  `a_late_shared_disagreement_after_agreeing_writes_is_rejected_and_retries_cleanly`).
- **Static-only cohort membership.** In `staged.rs`, `static_only` membership in the cohort
  (three call sites), and the duplicate checks of `static_color_targets` and
  `static_zoom_targets`, scanned vectors. Each took about 5 % of a hard-floor frame. They are
  sets now.
- **Captured Position program identity.** A captured Position program took a random v4 UUID,
  one `getentropy` call per animated owner and frame (2 %). It is now a process-unique counter
  identity (test: `captured_program_identities_are_unique_and_never_nil`). The program duplicate
  check is a set.

| Workload | Before | After |
| --- | ---: | ---: |
| Typed stress 2,000, 60 Hz | 161.4 ms | 130.6 ms |
| Typed hard floor 4,148, 125 Hz | 110.3 ms | 59.3 ms |

### For TL-553: reproducible failing workloads

Run each from the candidate root with `--fixture-package-dir assets/fixture-library
--protocol both --transport encode-only`.

1. **The typed capacity tiers are 8–20× over budget.** No single hotspot remains. In a CPU
   sample of typed stress 2,000, 91 % of the frame is in hybrid preparation, spread over:
   - family composition and observation;
   - Colour resolve;
   - typed sampling (`prepare_dynamic_family_samples_with_requirements`, `pin_samples`,
     `DeferredTypedSampling::complete`);
   - source binding;
   - the engine static frame, which `prepare_hybrid_frame` resolves twice per frame.

   Reproduce with `light-benchmark --headless-stress-fixtures 2000 --semantic --seconds 6`
   (and `4000`), and with
   `--profile hard-floor --rate-hz 125 --sustained-show --semantic`.
2. **Colour has no per-frame result reuse.** Unchanged static Colour bases are re-fitted every
   frame: 66 of 66 in the TL-564 mix, and 4,408 per frame at the hard floor. Reproduce with
   `--semantic-workload <TL-564 inputs> --static-bases-only --tracking-hz 60
   --tracking-scenario static-points`.
3. **Unrelated Intensity Dynamics defeat the Position memo.** The memo key holds the full native
   baseline, so an Intensity Dynamic on the same fixture forces a refit. At the hard floor, 53 of
   104 static Angle owners refit every frame because of their dimmer Dynamics. Reproduce with
   `--profile hard-floor --rate-hz 125 --sustained-show --semantic`.
4. **Non-converging static fits are re-solved forever.** With the fixture grid on the floor
   (`--rig-height-mm 500`), 7 static Target owners never reach a memo hit. Each frame costs
   about 4,400 candidate evaluations. At the default 9 m, and at 3 m (one owner), they converge.
   Reproduce with `--semantic-workload … --static-bases-only --rig-height-mm 500
   --tracking-hz 60 --tracking-scenario static-points`.
5. **Memory roughly doubles with typed lanes** (table above). No bound has been decided.
6. **The legacy-workload regression** (+0.7–2.1 ms p50) is in engine-wide code paths, listed
   above. Reproduce with `node tools/run-semantic-output-benchmark.mjs --suites legacy`.

## Semantic output optimisation, October 2026 (TL-553)

The question was whether the TL-596 workloads could be brought within their deadlines without
changing semantics. Not yet: every output is still identical, the dirty-work gates and the 2,000
and hard-floor legacy gates now pass, and the TL-564 mix holds all 27 configurations. The typed
capacity tiers improved by 7-20 % and still miss by roughly 7x (stress 2,000) and 6x (hard floor).
What remains is the per-frame design of typed sampling and composition, described below with
reproducible workloads, not one more hotspot.

### Gates

Same runner, thresholds and host as TL-596 (`tools/run-semantic-output-benchmark.mjs`; legacy
five alternated rounds against the pre-semantic baseline, typed tiers and the TL-564 matrix three
rounds). Pipeline ms, medians.

| Gate (existing threshold) | TL-596 | TL-553 |
| --- | --- | --- |
| Paired p99 regression ≤ max(1 ms, 5 %), stress 2,000 / 60 Hz | fail, +1.20 | **pass**: 5.57 → 6.08 (+0.51) |
| Paired p99 regression, stress 4,000 / 60 Hz | fail, +2.13 | **fail**: 10.99 → 12.71 (+1.72) |
| Paired p99 regression, hard floor 4,148 / 125 Hz | pass, +0.78 | pass: 4.23 → 4.52 (+0.29); 125 Hz held 5 of 5 |
| Typed stress 2,000 / 60 Hz: rate held, 0 misses | fail, p99 133.6 | **fail**: p99 122.95, 8.8 Hz |
| Typed stress 4,000 / 60 Hz | fail, p99 296.0 | **fail**: p99 262.8, 4.0 Hz |
| Typed hard floor 4,148 / 125 Hz | fail, p99 65.2 | **fail**: p99 50.3, 23.0 Hz |
| TL-564 mix, 27 configurations: rate held, 0 misses | 25 pass | **27 pass** (p99 6.6-8.0 / 5.4-5.8 / 4.2-4.4 ms at 44 / 60 / 125 Hz) |
| No generation change or compile from motion | pass | pass |
| `static-points`: 0 Position fits | pass | pass |
| `small-subset` / `all-points-move`: fits and dirty set equal the dependents | pass | pass (6 / 24) |
| Unchanged Colour targets skip solves | fail, 66 of 66 refit | **pass**: 0 fits, 66 exact replays per frame |
| Readout consumers do not multiply physical solves | pass | pass, now compared by Colour resolves (see below) |
| Non-converging static Target fits (`--rig-height-mm 500`) | 7 fits, 4,380 evaluations per frame | **0 fits**, 54 memo hits |

The readout-consumer gate compared Colour *fits* per frame. A fit is now either solved or replayed
from unchanged inputs, and how many frames replay depends on the sampled Dynamics timeline, which
the unpaced warmup shifts by a few ticks per run (14,424 against 14,426 fits in 240 frames, with
15,840 resolves in both). The gate therefore compares Position fits, Colour resolves and optics
resolves, still exactly; Colour fits stay in the observed row.

Typed tiers before and after this work, the TL-596 final binary (`06ef7036…`) alternated with
the final candidate, three rounds, medians (`before-after-typed/`):

| Profile | Before p50 / p99 | After p50 / p99 |
| --- | ---: | ---: |
| Typed stress 2,000, 60 Hz | 125.16 / 131.64 | 118.26 / 126.89 |
| Typed stress 4,000, 60 Hz | 276.32 / 283.69 | 256.25 / 261.95 |
| Typed hard floor 4,148, 125 Hz | 55.08 / 59.40 | 43.95 / 49.57 |

### What changed

Every change keeps outputs identical; each has focused tests that compare against the
unoptimised path and change every key input.

- **Colour result reuse.** A destination head replays its last semantic fit when the intent and
  every raw channel its fit reads (`CompiledColorFitting::head_input_channels`: the Colour-owned
  controls plus every input of the head's forward model) are unchanged, and the whole native
  vector passes the fitter's range check (`accepts_raw`). Intensity is not an input, so an
  Intensity Dynamic no longer forces a refit. The memo lives in the head's scratch, which a new
  runtime generation recompiles. A replay publishes the same writes, achieved output, quality and
  continuity with zero work counters, and counts `result_reuses` (`color/resolve.rs`; tests:
  `color/tests/result_memo.rs`, updated `fitting_bench` and `color_router` assertions).
- **Position memo keyed on the fit's inputs.** The accepted-fit memo compared the full native
  baseline. It now compares native values and availability only at the axis drivers' channels,
  which are the only channels a fit reads, plus the fit's whole-vector range check. At the hard
  floor all 104 static Angle owners now reuse their fits (`position/fit_cache.rs`; tests:
  `exact_fit_input_comparison…`, `an_unrelated_intensity_change_keeps_the_accepted_fit_identical_to_a_fresh_fit`).
- **Unreachable static Targets.** A bounded search that finds no solution
  (`UnreachableTarget`) is a pure function of the memo key, so it is memoised with its held
  status like a fit; other held statuses are not (test:
  `an_unreachable_static_target_is_reused_with_its_held_status`).
- **One scalar resolution fewer per hybrid frame.** The scalar Current source resolved the
  captured frame a second time. Without missing fixed bases and without any Freeze it now reads
  the static lane already prepared over the same samples (`TickSources::over_static`,
  `PreparedOutputFrame::freezes_nothing`; test:
  `static_lane_scalar_sources_equal_the_observed_resolution_and_freeze_keeps_observing`). The
  remaining two resolutions are genuinely different inputs (with and without the scalar Dynamic
  samples).
- **Physical projection of unchanged instances.** A pooled frame instance whose final native
  values equal the ones its forward results describe keeps them (`physical_projection.rs`; tests:
  `unchanged_native_values_reuse_forward_results_identical_to_a_fresh_evaluation`,
  `a_rejected_physical_layout_never_reuses_stale_forward_results`).
- **Native raw capture once per root and token.** Every head of a multi-head root captured the
  whole root again; the static token now keeps each root/instance vector (test:
  `repeated_native_captures_of_one_token_equal_fresh_captures`).
- **Hashing.** Per-frame maps in typed preparation, retained-tape import, expression traversal,
  Angle forests and the projection plans use the Fx hasher; none is iterated in an
  order-dependent way.
- **Benchmark harness.** The legacy stress sampler cloned the lane's owner key twice per sample
  inside the timed region since TL-596; it borrows one key per lane again, as the baseline did.

### Where the typed frame's time goes now

CPU samples of the final binary (`sample(1)`), share of the frame:

- Typed stress 2,000 (≈118 ms): per-target family composition and observation 25 % (source
  projection 9 %, Colour resolve 6 %, retained-family composition 5.5 %); the other per-cohort
  observer passes 12 %; typed sample preparation 21 % (expression validation, Angle-forest
  bundling, owner splitting); deferred typed sampling 6 %; cohort finish 5 %; source binding
  4 %; pinning 2 %; the two static resolutions 5 %; the final render 7 %. Allocation, free and
  copies are 31 % of all samples, spread through the above.
- Typed hard floor (≈44 ms): the two static resolutions 18 % (7.8 ms, Programmer resolution
  dominates); the final render 16 %; family composition and observation 22 % (native raw capture
  for the Colour key 5.6 %); native family row installation 7 %; the rest spread thin.

### Why the deadlines need a redesign

The typed frame recompiles what the operator programmed on every frame:

1. Every animated Dynamic sample is a fully materialised expression. Preparation validates it by
   building a whole retained tape, prunes and splits it by owner, bundles Angle forests and
   compiles coupled expressions, all per target, lane and frame, because values change every
   frame (`light-dynamics` `programming/preparation.rs` documents this as "a correctness bridge,
   not a no-allocation or no-compilation frame-path guarantee").
2. Composition, observation, source-occurrence binding and native row installation then run per
   target with fresh allocations (31 % of samples are the allocator).
3. The scalar-resolved token is a second complete resolution of the show, although it differs
   from the static lane only by the scalar Dynamic samples.

At stress 2,000 the typed overhead is ≈40 µs per animated target per frame against a budget of
≈3 µs once the scalar baseline is paid. Reaching it needs, at least: a compiled per-generation
program per lane and target that samples numbers per frame instead of rebuilding expressions;
retained per-target composition and observation state with no per-frame allocation; and an
incremental scalar overlay on the static lane instead of a second resolution. These change the
family sampling pipeline's data model, so they are design work, not tuning.

Reproduce each (from the candidate root, `--fixture-package-dir assets/fixture-library
--protocol both --transport encode-only`):

- `light-benchmark --headless-stress-fixtures 2000 --semantic --seconds 6` (and `4000`);
- `light-benchmark --profile hard-floor --rate-hz 125 --sustained-show --semantic --seconds 6`;
- `LIGHT_RENDER_PHASES=1` and `sample <pid> 8` on a running process give the two breakdowns above.

### The legacy stress 4,000 regression

Render-phase counters, one run each, µs per frame (`legacy-phases/`):

| Phase | Baseline | Candidate |
| --- | ---: | ---: |
| Fixture projection | 3,720 | 4,579 |
| Contribution merge | 676 | 968 |
| Playback resolution (now inside the prepared capture) | 263 | 274 |
| Prepared capture total | - | 293 |
| Pipeline p50 | 9,901 | 11,237 |

- About 0.33 ms p50 and 0.5 ms p99 of fixture projection is the new observational physical
  projection: a knockout build without it measured +0.86 ms p99 against the baseline. Evaluating
  forward models after send, or on demand per instance, would remove it from the output path;
  `RenderResult.physical` is read directly by Stage publication, freeze capture, adoption and
  readouts, so this is an API change, left for a decision.
- The rest of fixture projection and the contribution merge are the larger winner record
  (origin, family evidence and projected timestamp added to every slot: 128 bytes per slot now)
  and the per-head native-projection checks; there is no single further hotspot.

### Memory

Resident memory is bounded over time: the typed hard floor holds 583-592 MB from 72 to 704
frames. Typed lanes still roughly double it (stress 2,000: 287 → 654 MB; hard floor: 253 →
575 MB). The heap holds 2.6 M allocations: about 160 MB in 63 buffers of 1.7-3.4 MB, 33 MB in
≈4,100 per-fixture 8 KB blocks, and ≈200 MB of 16-768 byte objects. No duplicated slot table
was found (one per generation); the per-fixture compiled Colour fitters are not shared between
identical profiles, which is the first candidate for a reduction.

### Measurement identities

- Baseline: `f67c84e48`, source `85f4401a…`, binary `8a95e4a0…` (the TL-596 baseline).
- Before: the TL-596 final binary `06ef7036…` (`182471506`, the TL-596 tree).
- Candidate: `182471506` plus the TL-553 working tree, source `28a9c7c7…` captured at build time,
  binary `1e8c7f19…`, `--release --locked --no-default-features`.
- Evidence: `.artifacts/performance/semantic-output/tl553-final-20261003T061850Z/`
  (`legacy/`, `semantic/` with the gate tables in `summary.json`, `before-after-typed/`,
  `legacy-phases/`, `rig500-*`).
- The runner's consumer comparison and its test changed after the source capture; the gates
  were re-evaluated from the raw runs. Other agents shared the host; every comparison alternates.
