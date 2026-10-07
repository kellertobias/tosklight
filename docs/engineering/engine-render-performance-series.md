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

TL-639 below continues this work with a build-equivalence harness and new numbers.

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

## Semantic output: shared models, memoised Programmer, deferred physics (TL-639)

The question was whether typed semantic output could be compiled once and sampled per frame so
that the typed tiers meet their deadlines, with byte-identical output. The legacy paired gate now
passes on every tier, including stress 4,000, and memory is cut by roughly half to three quarters.
The typed tiers improved by 28-32 % (p50) and still miss: stress 2,000 runs at 11.3 Hz (p99
95 ms), the hard floor at 28.3 Hz (p99 41 ms). Every change was verified frame by frame against
the TL-553 build (below). A per-target compiled plan for composition and observation was not
built; the remaining time and what such a plan needs are listed below.

### Gates

Same runner, thresholds and host as TL-553. Legacy five alternated rounds against the
pre-semantic baseline; typed tiers and the TL-564 matrix three rounds. Pipeline ms, medians.

| Gate (existing threshold) | TL-553 | TL-639 |
| --- | --- | --- |
| Paired p99 regression ≤ max(1 ms, 5 %), stress 2,000 / 60 Hz | pass, +0.51 | **pass**: 6.03 → 6.21 (+0.18) |
| Paired p99 regression, stress 4,000 / 60 Hz | fail, +1.72 | **pass**: 12.77 → 12.73 (-0.04) |
| Paired p99 regression, hard floor 4,148 / 125 Hz | pass, +0.29 | **pass**: 5.15 → 4.52 (-0.63); 125 Hz held 5 of 5 |
| Typed stress 2,000 / 60 Hz: rate held, 0 misses | fail, p99 122.95, 8.8 Hz | **fail**: p99 95.18, 11.3 Hz |
| Typed stress 4,000 / 60 Hz | fail, p99 262.8, 4.0 Hz | **fail**: p99 208.42, 5.0 Hz |
| Typed hard floor 4,148 / 125 Hz | fail, p99 50.3, 23.0 Hz | **fail**: p99 41.05, 28.3 Hz |
| TL-564 mix, 27 configurations: rate held, 0 misses | 27 pass | 25 pass; all-points-move at 125 Hz had one late frame in the 30 and 120 Hz tracking runs (see below) |
| No generation change or compile from motion | pass | pass |
| Static bases: 0 Position fits, dependents 6 / 24, unchanged Colour skips solves | pass | pass |
| Readout consumers do not multiply physical solves | pass | pass |

TL-564 at 125 Hz, all-points-move: the pipeline p99 is lower (3.9-4.3 ms against 4.6-5.9 ms for
the TL-553 build), but both builds show isolated late frames. Five extra alternated rounds of the
two configurations (`repeat-125/`) gave the TL-553 build 2 misses in 10 runs and this build 4 in
10, each a single frame (max 6.9-9.2 ms) in an otherwise 4 ms run. TL-596 recorded the same two
configurations failing the same way. This is not attributed to a code path; it stays a fail.

Typed tiers, the TL-553 final binary (`1e8c7f19…`) alternated with this one, three rounds
(`before-after-typed/`):

| Profile | Before p50 / p99 | After p50 / p99 | RSS before → after |
| --- | ---: | ---: | ---: |
| Typed stress 2,000, 60 Hz | 125.03 / 137.95 | 89.83 / 104.17 | 649 → 372 MB |
| Typed stress 4,000, 60 Hz | 270.86 / 278.25 | 199.10 / 215.91 | 1,181 → 675 MB |
| Typed hard floor, 125 Hz | 50.91 / 58.42 | 34.82 / 39.60 | 575 → 281 MB |

Legacy resident memory, baseline → candidate: stress 2,000 237 → 66 MB, stress 4,000 454 → 112
MB, hard floor 214 → 77 MB.

### Equivalence

`light-benchmark --digest-ticks N` renders N unpaced logical ticks and prints, per tick, the
encoded DMX checksum and per fixture a hash of every resolved value, profile head value, physical
row (native raw values and every forward result) and accepted Color head/output. Dynamic instance
identities seed Random lanes and were random per process, so this mode derives them from the
definition (`DynamicRuntime::derive_instance_ids_from`) and samples instances in identity order;
two runs of one build are then identical. Each step was compared with the TL-553 build (built from
`8997670d0` plus only the harness files) over 20 ticks of: typed stress 2,000, typed hard floor,
legacy stress 2,000, legacy hard floor, and the TL-564 workload in four modes (small-subset 60 Hz
tracking, all-points-move 120 Hz, static bases, static bases with the grid at 500 mm). Every
fixture and tick, every DMX checksum and every Point pose matched. The only differing work
counter is `color_fitting_compiles` (fitters are now shared).

Preload, Freeze, FixAT and fades are not in these workloads; their equivalence rests on the
existing suites, all green, and the focused tests named below.

### What changed

- **Field scopes are bit sets** (`light-core` `field_trace/scope.rs`). Every trace query
  unioned, intersected and subtracted sorted `Arc<[field]>` sets, allocating on each operation.
  Unit fields are now bits; individual wheels and native channels keep a sorted list. Equality,
  ordering, iteration, Debug and serialization are those of the sorted list (test:
  `bit_scopes_match_the_sorted_slice_reference_on_every_operation`, 20,000 random pairs against
  the former implementation). Owner validation is a mask; owner keys are shared statics.
- **Physical forward results are evaluated on first read** (`physical_projection.rs`). The
  render captures native values and `complete`; Color, Position and Focus/Zoom forward models run
  when a reader calls `colors()`, `axes()`, `lenses()` or `optics()`, into recycled buffers, and
  unchanged native values keep earlier results. Readers are Stage native lanes (raw values only),
  freeze capture, Position and Zoom readouts, output API rows and Preload projections; all read
  after publication, off the output path (tests:
  `forward_results_are_evaluated_on_first_read_and_equal_a_fresh_evaluation`,
  `changed_native_values_are_never_read_through_stale_forward_results`, and the existing
  reuse/layout tests).
- **Shared profiles and models.** `FixtureDefinition.profile_snapshot` is an
  `Arc<FixtureProfile>`; fixtures compiled from one profile revision share it (it was deep-cloned
  per fixture: 225 MB in typed stress 2,000). Serialization is unchanged; in-place edits use
  `Arc::make_mut`. `CompiledModelInterner` shares compiled Colour fitters (headless adapter,
  across fixture lists) and Color/Position/Optics forward models (engine layout) between
  instances whose snapshot, mode, calibration, appearance and context are identical; the context
  compares by identity (tests: `identical_inputs_share_one_compile_and_any_difference_compiles_again`,
  `identical_fitting_inputs_share_one_fitter_across_fixtures_and_copies`,
  `identical_installations_share_compiled_forward_models`). Retained accepted Color frames no
  longer keep the sidecars' larger in-place allocation (110 MB across 32 frames).
- **Programmer evaluation memo** (`programmer_memo.rs`). Without fades, a Programmer's values
  before the sampled-replacement filter are a pure function of its captured vectors, the
  generation, the tracing flags and the transition history. The transition history now carries a
  process-unique content version; an evaluation that left it unchanged is kept and reused by the
  next static resolution and the next frame. The replacement filter and winner arbitration still
  run per call (tests: `kept_evaluations_equal_fresh_ones_with_and_without_sampled_replacements`,
  `fades_and_changed_histories_are_always_evaluated_again`).
- **Plain leaf samples** skip the generic tape import, pruning, owner split and classification
  traversals; their validation runs the leaf's own node checks (tests: `plain_leaves.rs`, each
  shortcut against the generic path).
- **Hashing and retains.** Programmer transitions, source-origin bindings, sampling work sets and
  binding membership sets use the Fx hasher (never iterated order-dependently). Retaining
  bindings by key or by authored binding no longer looks up a record for every static binding
  (test: `key_and_authored_retains_equal_the_record_retain`).

### Where the typed frame's time goes now

CPU samples of the final binary (`sample(1)`), share of the frame:

- **Typed stress 2,000 (≈88 ms).**
  - Family composition and observation, 42 %:
    - observation 13 % (Colour resolve 7 %, field projection 4 %);
    - retained composition 8 %;
    - the Position observer's program composition and finish 11 %.
  - Typed preparation 19.5 %. Position forest bundling and coupled compilation alone are 8.5 %,
    for two plain Pan/Tilt leaves per target.
  - Deferred typed sampling 6.6 %; cohort finish 5.5 %; pinning 3 %; source binding 3 %.
  - Final render 7 %; static resolutions 2 %.
  - The physical math is small: Colour fits 3.2 %, Position fits 0.4 %.
  - Allocation, free and copies are about 19 %.
- **Typed hard floor (≈35 ms).**
  - Final render 19 %, of which the per-head value maps (`prepare_head_inputs`) are a quarter.
  - Family observation 24 %, of which Colour resolve is 12 %: it replays memoised fits, but must
    capture the fixture's native raw values for the memo key (7 %).
  - Static resolutions 7.7 %, Programmer contributions 6 %.
  - Native family rows 6.5 %; family row projection 2.5 %; accepted Color record 2.8 %.

### What reaching the deadlines needs

The budget is 16.7 ms (stress 2,000) and 8 ms (hard floor); the scalar control frames are 5.5 and
3.5 ms. The remaining typed cost is the per-target pipeline itself, about 40 µs per animated
target at stress 2,000:

1. **A compiled per-target Position plan.** Building a forest, a retained tape and a coupled
   expression for two numeric leaves every frame is 8.5 %; the observer's composition and finish
   another 11 %. A plan compiled when the lane structure changes, with numeric leaves rebound per
   frame, needs a value-rebinding entry point on `CompiledCoupledExpression` that keeps lineage,
   origins and Current dependencies exact.
2. **Retained composition routes.** `BaseEvaluation` rebuilds its tasks, segments and trace arena
   per target and frame; only the leaf values change.
3. **Native capture reuse.** The hard floor captures every Colour fixture's native vector each
   frame only to confirm an unchanged memo key. Replaying the recorded value reads and reusing the
   vector when they are equal would remove most of the 7 %.
4. **Final render head maps.** `prepare_head_inputs` builds a hash map per head per frame; a dense
   per-head view would serve the hybrid and legacy paths alike.

Each of these changes a data model shared with Preload and the cut coordinator, so each needs the
same frame-by-frame equivalence evidence as this work. Round 2 (next section) did item 4 and
measured what the others still cost.

### Measurement identities

- Baseline: `f67c84e48`, binary `8a95e4a0…` (the TL-596 baseline).
- Before: the TL-553 final binary `1e8c7f19…` (`8997670d0`).
- Candidate: `8997670d0` plus the TL-639 working tree, binary `d09bbd1e…`,
  `--release --locked --no-default-features`; source manifest
  `.artifacts/tmp/tl639/identity/final-source-manifest.json`.
- Evidence: `.artifacts/performance/semantic-output/tl639-final-20261003T102423Z/`
  (`legacy/`, `semantic/` with the gate tables in `summary.json`, `before-after-typed/`,
  `repeat-125/`). Digest runs: `.artifacts/tmp/tl639/d-*.json`, compared by
  `.artifacts/tmp/tl639/cmp.py`.
- The host was quiet during the final campaign.

## Semantic output, round 2: indexed lookups, kept winners, numbered family owners (TL-639)

Round 2 asked for a per-target compiled plan: composition, observation, the Position forest and
coupled expressions compiled once per generation, so a frame only samples numbers into pooled
buffers. That plan was not built. What was built removes overhead the round-1 profile could not
see behind the larger items: quadratic scans, a per-head map rebuilt for every native channel,
name lookups forced by unnumbered family owners, Programmer winners arbitrated two or three times
per frame, and per-frame map rebuilds in the lane commit. Outputs are byte-identical to the
round-1 build on every digest workload, including a new lifecycle digest that covers fades,
FixAT, Freeze, Preload GO and the output masters. The typed deadlines are still missed.

### Gates

Same runner, thresholds and host as round 1, final binary `630e5382…`. Legacy: five
alternated rounds against the pre-semantic baseline (`f67c84e48`); typed tiers three rounds;
the TL-564 matrix one run per configuration. The host was **not** quiet: macOS
`mediaanalysisd` (about 115 % CPU), later `spotlightknowledged` (about 100 %) and another
worktree's debug desk ran throughout; no other cargo or rustc ran. Pipeline ms, medians.

| Gate (existing threshold) | TL-639 round 1 | Round 2 |
| --- | --- | --- |
| Paired p99 regression ≤ max(1 ms, 5 %), stress 2,000 / 60 Hz | pass, +0.18 | **pass**: 5.94 → 6.04 (+0.11) |
| Paired p99 regression, stress 4,000 / 60 Hz | pass, -0.04 | **pass**: 12.15 → 12.59 (+0.44) |
| Paired p99 regression, hard floor 4,148 / 125 Hz | pass, -0.63 | **pass**: 4.57 → 4.06 (-0.51); 125 Hz held 5 of 5 |
| Typed stress 2,000 / 60 Hz: rate held, 0 misses | fail, p99 95.18, 11.3 Hz | **fail**: p50 70.39, p99 85.22, 14.2 Hz |
| Typed stress 4,000 / 60 Hz | fail, p99 208.42, 5.0 Hz | **fail**: p50 154.0, p99 174.3, 6.5 Hz |
| Typed hard floor 4,148 / 125 Hz | fail, p99 41.05, 28.3 Hz | **fail**: p50 20.26, p99 28.29, 48.3 Hz |
| TL-564 mix, 27 configurations: rate held, 0 misses | 25 pass | 25 pass; static-points at 125 Hz with 60 and 120 Hz tracking had 1 and 2 late frames (see below) |
| No generation change or compile from motion | pass | pass |
| Static bases: 0 Position fits, dependents 6 / 24, unchanged Colour skips solves | pass | pass |
| Readout consumers do not multiply physical solves | pass | pass |

Legacy resident memory, baseline → candidate: stress 2,000 238 → 70 MB, stress 4,000 456 →
115 MB, hard floor 212 → 80 MB.

Typed tiers, the round-1 binary (`d09bbd1e…`, `61a62dc33`) alternated with this one,
three rounds (`before-after-typed/`):

| Profile | Round 1 p50 / p99 | Round 2 p50 / p99 | Rate | RSS |
| ---: | ---: | ---: | ---: | ---: |
| Typed stress 2,000, 60 Hz | 81.04 / 87.63 | 70.76 / 86.03 | 12.2 → 14.0 Hz | 372 → 366 MB |
| Typed stress 4,000, 60 Hz | 185.50 / 208.75 | 153.48 / 168.16 | 5.4 → 6.5 Hz | 680 → 689 MB |
| Typed hard floor, 125 Hz | 28.76 / 33.38 | 20.38 / 25.84 | 34.4 → 48.2 Hz | 278 → 264 MB |

The p50 falls by 13 % (stress 2,000), 17 % (stress 4,000) and 29 % (hard floor). The stress
2,000 p99 moved little because two of its six runs met the busiest moments of the host (single
runs 76.6-105.3 ms). Against the TL-553 binary the hard floor p50 has fallen from 50.9 to 20.4 ms.

### Equivalence

Every step was compared with the round-1 build (`61a62dc33` plus only the harness files below,
built separately) with `light-benchmark --digest-ticks`: the eight round-1 workloads (20 ticks)
and, new, `--digest-lifecycle` on typed and legacy stress 2,000 and hard floor (30 ticks). The
lifecycle run applies, before fixed ticks, a faded Programmer change of Color, Position and
Intensity, a FixAT of whole Color and Position families, a full and a partial Freeze with
installation Pan/Tilt inversion (a patch replacement, so also a generation change), a Preload GO
with a Programmer Fade, the Freeze release, then Grand Master at half, Blackout and control loss.
Both reference runs reproduce on every fixture and tick; every candidate matched on every
fixture, tick, DMX checksum, Point pose and work counter, and each event reported the same
outcome. The harness needed `LiveOutputBench::replace_snapshot` (the desk's prepare/install pair
for a changed patch); the reference build carries the same harness files.

### What changed

- **Head values read through the frame** (`profile_projection/head_values.rs`). A head on the
  overlay path, and every fitted native channel, copied its fixture's values and sequence masters
  into two hash maps. `HeadValueView`/`HeadMasterView` answer each lookup exactly as those maps
  did (an unnumbered value of a name wins over a numbered one) and keep only the head's own
  writes, inline. Without control loss, hazardous Blackout or a Highlight look, seeding a native
  candidate only replaces its own attribute. Test:
  `head_lookups_answer_like_the_head_maps_they_replaced` (dense frame with unnumbered values, and
  values handed over by name).
- **Kept Programmer winners** (`programmer_memo.rs`). Round 1 kept the evaluation; the
  replacement filter and winner arbitration still ran for every static resolution. The winners
  are a pure function of the kept evaluation and of the positions the filter removed, so each
  kept evaluation keeps up to four arbitrations by removed set and offers them borrowed. The key
  is the evaluation's own key (Programmer vectors by identity, generation, tracing and replacement
  flags, transition-history version); a new evaluation starts empty. Tests:
  `kept_winners_belong_to_one_evaluation_and_removed_set` (every input, removed sets, beyond the
  kept count) and `kept_winners_equal_a_fresh_arbitration_for_every_removed_set_and_flag`.
- **Family owners are numbered** (`frame_slots.rs`). `position`, `zoom` and `focus` join
  `intensity` and `color` as attributes every profile head numbers. A typed Position value used to
  land in the unnumbered overflow of every head that held one, and an overflowed head answers every
  channel read the patch did not number (the control attribute of every channel, first) by
  hashing the name. Arbitration of a numbered and an unnumbered value is the same function. Two
  engine tests used `position`/`zoom` as their example of an unnumbered pair; they now use an
  unpatched target and `iris`.
- **Quadratic scans removed.** Per target, the Position batch composer scanned every group
  (membership, twice per call), the Position observer scanned every program and every current
  cohort, and Position static targets used `Vec::contains`. Each is now an index built once per
  frame with the same first-match and duplicate rules.
- **Smaller per-frame work.**
  - The physical lane commits in place (entries stamped per accept; untouched entries are
    released) and lends the previous continuity to the adapter instead of cloning it.
  - Family sidecars are boxed: a 768-byte row moved through several vectors per frame.
  - Accepted Colour heads and outputs are sized exactly; native family writes are validated once
    per owner run; `same_static_baseline` reads each winner once (`StaticWinner`).
  - A Position-only forest decides it has no remainder by reading the tape instead of copying and
    compacting it; the Programmer colour set is only built when a Group colour exists.

### Where the time goes now

CPU samples of the final binary, share of the frame:

- **Typed stress 2,000 (≈70 ms).**
  - Family composition and observation, 40 %: Colour observation 13 % (Colour resolve 8 %, of
    which fitting about 4.5 %; source projection 4.3 %), retained composition about 7 %, the
    Position observer's program composition 5.4 % and program preparation 3.1 %, input assembly
    4.4 % (mostly freeing the previous frame's samples).
  - Typed preparation 22 %: the Position forest 8.8 %, owner samples 5.1 %, expression
    validation 2.9 %, the sample sort 1.7 %.
  - Deferred typed sampling 7.2 %, cohort finish 5.6 %, pinning 3.1 %, source binding 3.1 %.
  - Final render 5.6 %, static resolutions 2 %. Allocation and copies are about 19 %.
- **Typed hard floor (≈20 ms).**
  - Family composition 42 %, almost all of it the static Colour owner of every head: Colour
    resolve 12.6 % (native capture 5 %, the memoised fit 3 %), source projection 8.8 %,
    retained composition 4.4 %, static target discovery 3.1 %.
  - Final render 16 %, the two static resolutions 8 %, native family rows 7 %, accepted Colour
    record 2.3 %, pinning and source binding 5 %.

### Isolated late frames at 125 Hz (TL-564)

`LIGHT_BENCHMARK_SLOW_FRAME_MS=N` prints the phase breakdown of every frame over N ms.

- Late frames fall on different ticks in every run and are not tied to a tracking sample.
- In some, every phase slows at once: the capture, a fixed 0.03 ms of work, took 0.18-0.56 ms
  next to a 7-10 ms transaction. In others only the transaction is long (7.4-9.8 ms against
  about 4 ms).
- Ten alternated runs of each build at 125 Hz (all-points-move, 30 and 120 Hz tracking,
  `repeat-125/`) gave late frames in 4 runs for each build, 1-2 frames each. In this campaign's
  matrix the two late runs were static-points, not all-points-move.
- Clamped to the efficiency cores (`taskpolicy -c utility`), the same run has 61 late frames at a
  p50 of 3.8 ms.

The evidence points to the scheduling of a default-QoS output thread on a busy host
(descheduling, or a move to a slower core), not to a path this work changed. A frame-work cause
for the frames where only the transaction is long is not excluded. The next step is a real-time
or user-interactive QoS class for the output thread of the desk and the benchmark, which needs a
platform call. The runs stay counted as failures.

### What the compiled plan still needs

Measured by p99, the typed tiers are 5.1× (stress 2,000), 10.4× (stress 4,000) and 3.5× (hard
floor) from their budgets. No remaining item is larger than 9 %: the cost is spread over the
per-target pipeline, which builds and discards structures every frame. Reaching the budgets
needs the compiled plan, in this order of yield:

1. **Static family rows (hard floor, about 30 %).** A static-only Colour owner recomposes, projects
   its sources and resolves its native writes every frame for the same answer. A per-lane,
   per-target memo of (composed value, provenance, resolution, metadata) needs an exact change
   token for each input: the static winner (value `Arc` identity, change time, evidence and origin
   identity), the bound static-evidence occurrence, the previous continuity, and the native raw
   values of the head's Colour inputs. The last needs per-channel slot change detection; the
   whole-fixture native vector changes with Intensity every frame. The binding call must still
   run, because it keeps the occurrence alive.
2. **Typed preparation (stress, 22 %).** Animated lanes produce new expressions every frame, so the
   compiled-sample cache never hits. A plan keyed by expression *shape* with numeric leaves rebound
   needs a rebinding entry point for `FamilyCompositionSample` and, for the Position forest,
   `CompiledCoupledExpression` (round 1, item 1) that keeps lineage, occurrences and Current
   dependencies exact.
3. **Component edits over a static base (stress, about 11 %).** `BaseEvaluation` rebuilds its
   tasks, segments and trace arena for three recipe components per target (round 1, item 2), and
   the source projection replays the same trace query.
4. **The second static resolution (hard floor, 4 %).** The scalar lane re-offers every Playback and
   Programmer contribution only to add the Intensity samples; deriving it from the original lane
   needs the sampled-replacement filter to be applied as a delta.

Smaller, safe items found but not done: the cue and Playback source-binding passes scan every
authored binding twice per frame with a record lookup each (2 %); the per-instance Dynamics maps
use SipHash (1.5 %); `ColorIntent::validate` runs five times per target (2.5 %).

Found while profiling, outside this item: the first frame after starting Dynamics on 4,000
targets spends more than 10 s in `merge_dynamic_address_values`/`dynamic_conflicts`, which is
quadratic in the Programmer's Dynamic rows. It blocks Live output after a large start. Fixed in
round 3 (next section).

### Measurement identities

- Baseline: `f67c84e48`, binary `8a95e4a0…`.
- Before: the round-1 final binary `d09bbd1e…` (`61a62dc33`).
- Reference for equivalence: `61a62dc33` plus only the harness files (`digest.rs`,
  `digest_lifecycle.rs`, `mod.rs`, `semantic_arguments.rs`, `live_output_bench.rs`), built in
  `.artifacts/tmp/tl639/base`.
- Candidate: `61a62dc33` plus the round-2 working tree, binary `630e5382…`,
  `--release --locked --no-default-features`; source manifest
  `.artifacts/tmp/tl639r2/identity/final-source-manifest.json`.
- Evidence: `.artifacts/performance/semantic-output/tl639r2-final-20261003T131225Z/`:
  `legacy/`, `semantic/` (gate tables in `summary.json`), `before-after-typed/`, `repeat-125/`
  (with slow-frame logs). Digest runs:
  `.artifacts/tmp/tl639r2/{d,l}-*.json`, compared by `.artifacts/tmp/tl639r2/cmp.py`.

## Semantic output, round 3: linear Dynamic folds, kept static rows (TL-639)

Round 3 was asked to fix the quadratic first frame, then build the per-target compiled plan in
order of yield: memoised static family rows, numeric leaves rebound into compiled Position and
family samples, component edits planned over a static base, the second static resolution derived
from the first, and a real-time or user-interactive class for the output thread. The quadratic
fix and the static rows were built, each proven byte-identical with the digest harness. The
output thread class was built, measured, and reverted: it did not reduce late frames. The three
compiled-plan items for animated rows were not built. The typed deadlines are still missed.

### Gates

Runner, thresholds and reference as in round 2; candidate binary `f2a58342…`. Legacy: five
alternated rounds against `f67c84e48`; typed tiers three rounds; TL-564 one run per
configuration. Host: another worktree's debug desk (10-60 % CPU), the MOTU audio driver (about
12 %) and the desktop app ran throughout; no `mediaanalysisd` or Spotlight indexing, no other
cargo or rustc. Pipeline ms, medians.

| Gate (existing threshold) | Round 2 | Round 3 |
| --- | --- | --- |
| Paired p99 regression ≤ max(1 ms, 5 %), stress 2,000 / 60 Hz | pass, +0.11 | **pass**: 5.80 → 6.33 (+0.53) |
| Paired p99 regression, stress 4,000 / 60 Hz | pass, +0.44 | **pass**: 11.80 → 12.45 (+0.65) |
| Paired p99 regression, hard floor 4,148 / 125 Hz | pass, -0.51 | **pass**: 4.70 → 4.12 (-0.58); 125 Hz held 5 of 5 |
| Typed stress 2,000 / 60 Hz: rate held, 0 misses | fail, p99 85.22, 14.2 Hz | **fail**: p50 71.72, p99 91.38, 13.8 Hz |
| Typed stress 4,000 / 60 Hz | fail, p99 174.3, 6.5 Hz | **fail**: p50 155.87, p99 177.76, 6.5 Hz |
| Typed hard floor 4,148 / 125 Hz | fail, p99 28.29, 48.3 Hz | **fail**: p50 18.68, p99 23.56, 52.5 Hz (315 misses per round) |
| TL-564 mix, 27 configurations: rate held, 0 misses | 25 pass | 24 pass (see below) |
| No generation change or compile from motion | pass | pass |
| Static bases: 0 Position fits, dependents 6 / 24, unchanged Colour skips solves | pass | pass |
| Readout consumers do not multiply physical solves | pass | pass |

The three TL-564 failures: static-points, 30 Hz tracking at 44 Hz output, dropped 3 ticks with
no frame over 7.1 ms (the scheduler did not run on time); all-points-move at 60/60 Hz and
static-points 30 Hz tracking at 125 Hz had 1 and 2 late frames. Different configurations fail
in every campaign. Ten alternated 125 Hz all-points-move repeats (`repeat-125/`): round-2 code
late in 7 of 20 runs (9 frames), this build in 10 of 20 (16); a separate five-round A/B of the
same workload gave equal pipeline p50 (2.8-3.2 ms both) and 5 late frames for round 2, 2 for
this build.

Typed tiers, round-2 code (`a4d93948c`, rebuilt) alternated with this one, three rounds
(`before-after-typed/`):

| Profile | Round 2 p50 / p99 | Round 3 p50 / p99 | Rate |
| ---: | ---: | ---: | ---: |
| Typed stress 2,000, 60 Hz | 71.35 / 87.97 | 71.85 / 91.95 | 13.9 → 13.7 Hz |
| Typed stress 4,000, 60 Hz | 155.66 / 186.07 | 158.88 / 166.95 | 6.4 → 6.3 Hz |
| Typed hard floor, 125 Hz | 20.25 / 28.64 | 18.60 / 25.80 | 48.0 → 52.7 Hz |

The hard floor p50 falls 8 %. The stress tiers are unchanged within the host's spread (single
stress 2,000 p99 runs 77.6-101.0 ms); their profiles contain no measurable share of the new code.
Resident memory: typed 370 → 373 MB (stress 2,000), 263 → 273 MB (hard floor).

### The quadratic first frame

Starting a Dynamic on 4,000 targets took 14.2 s for the first frame and 19.4 s for the whole
benchmark start; Live output was blocked for that time.

- `merge_dynamic_address_values` (the Off/On precedence fold of one Programmer's Dynamic rows)
  compared every row with every other row. A row is dropped by a conflicting row that is later,
  or that precedes it in storage without being earlier. Every row that can conflict with a row
  lies in one of at most three buckets of that row (same fixture, owner and track key; whole-owner
  Release against unlinked rows; an instance against its Off rows), and inside a bucket "some
  member drops this row" reduces to a few maxima over ordered and unordered (legacy zero-order)
  members. The fold is now O(n log n); inputs of up to 24 rows keep the pairwise scan.
- The same start spent 3 s in `ProgrammerRegistry::apply_dynamic_values`, which removed the rows
  each mutation replaces by scanning all stored rows, under the Programmer lock. A Live gesture
  over many targets now applies through buckets of stored rows (every member of a used bucket is
  removed, so each row is visited a bounded number of times). Preload keeps the row-by-row path:
  its released-colour bookkeeping reads the rows between mutations.
- Result at 4,000 targets: first frame 14.2 s → 0.68 s (2,000 targets: 0.25 s), benchmark start
  19.4 s → 2.0 s. The remaining first frame is the generation's one-off compile (adapter
  descriptors, profile head destinations, projection plans).
- Tests: `indexed_fold_keeps_exactly_the_rows_the_pairwise_relation_keeps` (600 random cases
  with ties, legacy zero orders, Off, Release, component holds, storage orders) and
  `fold_time_grows_near_linearly_with_target_count` (25 µs per target bound; the pairwise fold
  needs 64 ms at 1,000 targets and fails it); `indexed_gesture_leaves_the_rows_and_change_answer_of_the_row_by_row_path`
  (400 random gestures, rows and the "changes" answer) and
  `large_gestures_apply_in_time_proportional_to_their_targets` (25 µs per target; the
  row-by-row path takes 53 ms at 1,000 targets).
- `origin/main` (`f67c84e48`, as fetched in this worktree) does not have this fold: it
  reconciles Programmer Dynamics by grouping rows per instance link in one pass. The quadratic
  path came with this branch's Off/On precedence rules.

### Equivalence

Every step was compared with `a4d93948c` (built separately) by `--digest-ticks` on the eight
round-1 workloads (20 ticks) and `--digest-lifecycle` on typed and legacy stress 2,000 and hard
floor (30 ticks): every fixture, tick, DMX checksum, Point pose and work counter matched, and
every lifecycle event reported the same outcome. The whole-target Colour replay increments the
Colour counters exactly as the per-head replays it stands for, so the work counters stay equal.

### What changed

- **Kept static-only compositions** (`programming_projection/static_rows.rs`). A static-only
  owner (Colour or Zoom with no Dynamic samples, kept so the adapter fits its static value)
  recomposed its base every frame. Composition without samples reads only the owner, the base and
  the fixed authoring model: it adopts nothing, samples no Current and resolves no transition. The
  composed value and its trace are kept per lane, target and owner while the base is equal; Position
  is excluded (its base composition can adopt through the frame geometry). Tests:
  `a_kept_row_is_the_composition_of_its_base_and_recomposes_only_when_the_base_changes` (with a
  frame resolver that panics if consulted) and
  `a_failed_composition_keeps_nothing_and_unused_rows_leave_with_their_cohort`.
- **Kept source projections.** The observer's projection of a kept row reads the trace query, the
  bound static-evidence occurrence and that occurrence's record. It is answered again while the
  requested fields and the freshly bound occurrence are equal and the record catalogue is the same
  allocation (`DynamicSourceOrigins::records_identity`: records are immutable, and a held `Weak`
  moves the catalogue on its next write). The binding still runs every frame.
- **Whole-target Colour replay** (`physical_adapter/color/target_memo.rs`). A head replays its
  memo while the intent and its fit inputs of the seeded native vector are unchanged and the vector
  passes the fitter's range check. When the intent, the previous continuity and the native values
  of every head's inputs and controls are unchanged, and the raw vector passes the range check,
  every head would replay; the target returns the kept result with zeroed work, as those replays
  publish it. Intensity and other channels outside the key may change. It is kept only from the
  second resolve of an unchanged intent on, so animated targets pay one comparison. Test:
  `an_unchanged_target_replays_whole_and_any_key_change_resolves_again`.

### Output thread class (not kept)

Ten alternated rounds of TL-564 all-points-move at 125 Hz (30 and 120 Hz tracking, 20 runs per
class) with one binary, the measuring thread in each class:

| Class | Runs with late frames | Late frames | p99 median |
| --- | ---: | ---: | ---: |
| Default | 6 / 20 | 9 | 4.24-4.26 ms |
| User-interactive QoS (`pthread_set_qos_class_self_np`) | 8 / 20 and 10 / 20 | 9 and 15 | 4.18-4.34 ms |
| Real-time (Mach time-constraint policy, computation half the period) | 19 / 20 | 4,243 | 10.83 ms |

User-interactive QoS did not reduce late frames; the real-time policy made every run late. The
late frames take 8.1-12 ms against a 3-4 ms median, also with user-interactive QoS. A
scheduling cause is still likely for some (the dropped ticks above, captures of 0.26-0.53 ms
instead of 0.03 ms) but a class request does not remove them. The change (a platform crate with
the only `unsafe` call, the desk's output thread and a benchmark option) was reverted; the
attempt is kept under `.artifacts/tmp/tl639r3/qos-attempt/`. The repository has no existing
platform abstraction for thread scheduling, and the desk runtime, output and benchmark crates
forbid `unsafe`.

### Not built, and where the time goes now

CPU samples of the final binary, share of the frame:

- **Typed stress 2,000 (about 71 ms).** Family composition 41 %: composition of animated rows
  25 % (retained composition 9.5 %, of which base evaluation 8.7 %; Colour observation 15 %, of
  which the Colour fit 4.8 %), the Position observer's program composition 4.7 % and cohort fit
  3.8 %. Typed preparation 22 % (Position forest bundling 8.1 %, expression validation 3 %).
  Deferred sampling 7.6 %, cohort finish 5.6 %, final render 6.5 %, pinning and binding 6.5 %.
  Freeing and copying the per-frame structures: about 21 %.
- **Typed hard floor (about 18.6 ms).** Static Colour rows 23 % (native capture 5.6 %, source
  binding 4.8 %, field scopes with intent validation 3.7 %, the replayed result's copy 2.7 %),
  final render 21 %, the two static resolutions 8.6 %, native
  family rows 7.5 %, static target discovery 3.4 %, accepted Colour record 2.5 %.

The three items not built carry the stress tiers' remaining distance, and each needs a design
that keeps lineage exact:

1. **Numeric leaves rebound into compiled samples (stress, about 22 %).** The Position forest
   (`bundle_position_component_forest`) builds a tape, a forest and a `CompiledCoupledExpression`
   per target and frame, and the lineage it carries (`PositionForestLineage`: tapes, whole and
   cohort-member maps, the retained samples) is read later by the Position observer's cut
   coordinator and resume paths. A plan keyed by expression shape needs a mapping from every tape
   leaf to its compiled node and lineage entry, and a proof that a rebound plan's lineage equals a
   fresh build; neither exists yet.
2. **Component edits over a static base (stress, about 9 %).** `BaseEvaluation` rebuilds its
   tasks, segments and trace arena for the recipe components of every animated row.
3. **The second static resolution (hard floor, about 9 %).** It re-offers every Playback and
   Programmer contribution to add the Intensity samples. Deriving it needs the sampled-replacement
   filter, the Programmer underlay and Move-in-Black (which read the sampled values) applied as a
   delta, and arbitration ties must not depend on offer order.

Smaller items seen: the static-evidence binding looks its record up in a `BTreeMap` of all records
per target and frame (4.8 % hard floor); `ProgrammingFieldScope::for_value` validates the whole
Colour intent for every field-scope query (3.7 %).

Round 4 (next section) built D and E in part and the frame-level replays; A, B and C remain.

### Measurement identities

- Baseline: `f67c84e48`, binary `8a95e4a0…`.
- Before and digest reference: `a4d93948c` built in `.artifacts/tmp/tl639/base`, binary
  `0133953b…` (`.artifacts/tmp/tl639r3/bin/lb-base`).
- Candidate: `a4d93948c` plus the round-3 working tree, binary `f2a58342…`
  (`.artifacts/tmp/tl639r3/bin/lb-final`), `--release --locked --no-default-features`; source
  manifest `.artifacts/tmp/tl639r3/identity/final-source-manifest.json`.
- Evidence: `.artifacts/performance/semantic-output/tl639r3-final-20261003T151215Z/`
  (`legacy/`, `semantic/`, `before-after-typed/`, `repeat-125/`); output-class runs
  `.artifacts/tmp/tl639r3/qos-{1,2}/`. Digest runs `.artifacts/tmp/tl639r3/{d,l}-*.json` and
  summaries `digest-*.txt`, compared by `.artifacts/tmp/tl639r3/cmp.py`.

## Semantic output, round 4: kept installations, shared captures, pooled frames (TL-639)

Round 4 was asked for the design-level change rounds 1-3 avoided: per-target compiled plans with
exact lineage (A), component edits planned over a static base (B), the second static resolution
derived as a delta (C), static Colour rows without a full native capture (D), and a flat final
render (E), with pooled buffers so steady-state frames allocate almost nothing. The design was
written first (TL-639 handoff, round 4). What was built is part of D and E, the pooling, and a
set of exact frame-level replays that each prove equality from unchanged inputs; A, B and C
were not built (reasons below). Every step was byte-identical to `71a68e1d1` on all 12 digest
workloads. The typed deadlines are still missed: the hard floor p50 fell 23 % (18.5 → 14.3 ms)
and stress 2,000 9 % (70.4 → 63.8 ms).

### Gates

Runner, thresholds and references as in round 3; candidate binary `ba02fa6e…`. Legacy: five
alternated rounds against `f67c84e48`; typed tiers three rounds; TL-564 one run per
configuration. Host: quiet (the idle desk on port 5000 was stopped; no other build or desk).
Pipeline ms, medians.

| Gate (existing threshold) | Round 3 | Round 4 |
| --- | --- | --- |
| Paired p99 regression ≤ max(1 ms, 5 %), stress 2,000 / 60 Hz | pass, +0.53 | **pass**: 5.49 → 5.72 (+0.23) |
| Paired p99 regression, stress 4,000 / 60 Hz | pass, +0.65 | **pass**: 11.67 → 11.52 (-0.16) |
| Paired p99 regression, hard floor 4,148 / 125 Hz | pass, -0.58 | **pass**: 4.55 → 3.74 (-0.81); 125 Hz held 5 of 5 |
| Typed stress 2,000 / 60 Hz: rate held, 0 misses | fail, p99 91.38, 13.8 Hz | **fail**: p50 63.80, p99 69.57, 15.7 Hz |
| Typed stress 4,000 / 60 Hz | fail, p99 177.76, 6.5 Hz | **fail**: p50 135.79, p99 140.93, 7.5 Hz |
| Typed hard floor 4,148 / 125 Hz | fail, p99 23.56, 52.5 Hz | **fail**: p50 14.28, p99 17.26, 68.8 Hz (about 414 misses per round) |
| TL-564 mix, 27 configurations: rate held, 0 misses | 24 pass | 25 pass (see below) |
| No generation change or compile from motion | pass | pass |
| Static bases: 0 Position fits, dependents 6 / 24, unchanged Colour skips solves | pass | pass |
| Readout consumers do not multiply physical solves | pass | pass |

Resident memory, typed: 369 MB (stress 2,000), 682 MB (stress 4,000), 272 MB (hard floor).

The two TL-564 failures are static-points at 125 Hz output with 30 and 120 Hz tracking: 3 and 1
frames over the period (12.96 and 9.67 ms) in runs whose p99 is 3.7 ms. They are the isolated
late frames of rounds 2 and 3 (different configurations fail in every campaign). In ten
alternated 125 Hz all-points-move repeats (`repeat-125/`), `71a68e1d1` had late frames in 3 of 20
runs (3 frames) and this build in 1 of 20 (2 consecutive frames, transaction 8.9 and 12.2 ms);
the pipeline p99 medians are 3.75 and 3.67 ms.

Typed tiers, `71a68e1d1` (rebuilt) alternated with this build, three rounds
(`before-after-typed/`):

| Profile | `71a68e1d1` p50 / p99 | Round 4 p50 / p99 | Rate | RSS |
| ---: | ---: | ---: | ---: | ---: |
| Typed stress 2,000, 60 Hz | 70.41 / 79.00 | 63.77 / 70.70 | 14.1 → 15.6 Hz | 373 → 369 MB |
| Typed stress 4,000, 60 Hz | 150.41 / 157.03 | 136.77 / 145.06 | 6.6 → 7.3 Hz | 686 → 681 MB |
| Typed hard floor, 125 Hz | 18.54 / 21.67 | 14.30 / 16.78 | 53.0 → 68.7 Hz | 280 → 270 MB |

The p50 falls 9 % (stress 2,000 and 4,000) and 23 % (hard floor). By p99 the typed tiers are
4.2× (stress 2,000), 8.4× (stress 4,000) and 2.2× (hard floor) from their budgets.

### Allocation per frame

Measured with a counting global allocator wrapped around the unchanged benchmark modules
(`.artifacts/tmp/tl639r4/alloc-count/`, its own workspace; the benchmark binary forbids
`unsafe`), as the difference between a 2 s and a 5 s run divided by the extra frames:

| Workload | `71a68e1d1` | Round 4 |
| --- | ---: | ---: |
| Typed hard floor | 71,673 objects, 49.9 MB | 54,497 objects, 24.5 MB |
| Typed stress 2,000 | 605,566 objects, 143.0 MB | 535,783 objects, 122.5 MB |
| Legacy hard floor | 99 objects, 10.4 MB | 98 objects, 9.6 MB |
| Legacy stress 2,000 | 2,624 objects, 23.1 MB | 2,623 objects, 22.2 MB |

The bytes fell by half on the hard floor: the Programmer transition history (3.2 MB) was copied
three times per frame, and the lane staging, the native write list and the cohort's projection
list were regrown from empty every frame. The count fell 24 %. The remaining allocations are the
per-target structures the typed path builds and drops every frame (A and B); stress 2,000 still
allocates about 270 objects per target per frame.

### What changed

- **Kept native family installation** (`native_family_projection.rs`, `FamilyNativeMemo`). What
  `project_family_native` validates and installs depends on the runtime generation and the
  write list, except that every addressed owner needs a captured semantic value on the frame's
  token. A lane keeps its last installation; equal writes under the same generation re-check
  each owner's semantic value on the new token (same order, same error) and install the kept
  collection by reference (`NativePositionProjection.instances` is shared). Test:
  `a_kept_native_installation_is_the_full_installation`.
- **Static Colour rows replay from their key channels** (`native_raw_channels_into`,
  `ColorTargetMemo::replay_key`). The whole-target replay needed the full native vector only
  for the fitters' whole-vector range check. A capture resolves every non-static channel through
  `scale_channel_raw`, which never exceeds the channel maximum, and every static channel to its
  fixed default, so that check has one answer per destination and generation; it is kept with
  the memo. An unchanged target now resolves only the channels its fits read. Test:
  `a_channel_subset_reads_exactly_the_complete_capture`.
- **One capture per destination without inversion** (`native_raw.rs`). The Position capture of
  a root or copy without installation inversion reads exactly what the family-agnostic capture
  reads; both share one cache entry instead of resolving the fixture twice.
- **Kept observation derivations** (`static_rows::KeptObservation`). The consumed fields of a
  kept static row (a function of owner and value) and the control sources of a field scope (a
  function of the kept trace) are kept with the row.
- **Fitted native channels on a plain input path** (`profile_projection.rs`,
  `NativeChannelInputs`). Without control loss, hazardous Blackout, Highlight look or axis
  inversion, and when the scalar Freeze check keeps its candidate, a fitted native channel reads
  exactly its own value and the head's Intensity. It is answered without building the
  per-channel input set; the overlay state is resolved once per head and Freeze flag.
- **Shared Programmer transition history** (`programmer_memo.rs`). The history is copied on the
  first change, not on every capture and static resolution; an unchanged `retain` or an absent
  removal changes neither the map nor its version. Test:
  `transition_clones_share_the_history_until_one_changes`.
- **Pooled frame storage.** The physical lane reuses its staging lists, the family lanes their
  native write list; the cohort's projection list and the token's projection map are sized once.
- **Lookups.** The source record catalogue is Fx-hashed (`snapshot` sorts by occurrence);
  canonical attribute names answer their id by allocation identity before hashing (test:
  `shared_canonical_names_answer_with_the_number_of_their_name`); each fixture's profile mode is
  found once per generation (`RuntimeGeneration::fixture_mode`); lane selections are
  Fx-hashed and read once per controller and lane instead of per target and lane; the sample
  sort gathers its keys once; a captured family base is checked for presence without copying
  it out; plain expression leaves (including Angle leaves) compare, prune, split and report
  Angles without walking or allocating.
- Built, measured, reverted: comparing the Colour target memo's intent by program allocation
  before value (no measurable difference).

### Not built

- **A, per-target compiled plans.** The Position forest (8.5 % of stress 2,000), the observer's
  program preparation and composition (9 %) and the cohort finish (5 %) rebuild their structures
  from fresh samples every frame. A plan needs a structural shape key over the sample tapes, a
  slot map from every tape leaf to its compiled node and lineage entry, and an instantiate
  path for `CompiledCoupledExpression` and `PositionForestLineage` whose result is provably equal
  to a fresh build, including the tape validation and operation-origin table the bundle performs
  today. The validation alone is not a pure function of the leaf values (operation origins move
  into the tape's object table), so a rebound plan cannot skip it without a separate proof. This
  is the largest remaining item and was not started.
- **B, component edits over a static base** (9.6 %). `BaseEvaluation` moves large task structs
  and rebuilds segments and trace arenas; only `compose_segment` (4 %) is the edit itself. Not
  built for the same reason as A: the trace arena is read by the source projection.
- **C, second static resolution as a delta** (5.4 % hard floor). Its offers equal the first
  resolution's only when no sampled batch replaces a source and the Programmer contributions are
  the same kept winners; then a copy of the first resolver after its Playback and Programmer
  offers would serve. That copy clones the whole dense frame (every slot, about 1.7 MB on the hard
  floor), which costs about what the re-offer does. Not built.
- **Animated Colour targets** still capture every channel (2.8 % stress): the fitters need only
  the key channels, but the Position fit of the same destination shares the full capture, so a
  key-channel capture for Colour alone saves nothing until Position reads a subset too.

### Where the time goes now

CPU samples of the final binary, share of the frame:

- **Typed stress 2,000 (about 64 ms).** Family composition and observation 44 %: the animated
  Colour rows' composition 9.6 % (`BaseEvaluation`; the edit itself 4 %) and observation 16 % (the
  Colour fit 5.1 %, the full native capture 2.8 %, source projection about 4 %); the Position
  observer's program composition 5.6 % and preparation 3.7 %; input assembly 4.4 % (freeing the
  previous frame's samples). Typed preparation 21.5 % (Position forest 8.5 %, owner samples 5.1 %,
  expression validation 3.3 %). Deferred sampling 7.8 %, cohort finish 4.8 %, final render
  7.6 %, pinning 2.1 %. Allocation, freeing and copying are about 28 % of the samples. The
  physical math (Colour and Position fits) is about 6 %: an ideal single-threaded pipeline that
  kept only it and the render would still need the compiled plans (A, B) to approach 16.7 ms.
- **Typed hard floor (about 14.3 ms).** Static Colour rows 24 % (key-channel capture 5.6 %,
  source binding and projection 4.9 %, the replayed result's copy and staging about 4 %), final
  render 19 % (plus lane acceptance 2.5 %), the two static resolutions 10 %, the Intensity
  Dynamics' pinning, completion and preparation 8.6 %, static target discovery 4.2 %, accepted
  Colour record 3.3 %, row projection 2.8 %, the static-baseline guard 2.3 %. The legacy hard
  floor renders the same rig in 3.7 ms; the typed overhead left is the per-row work on unchanged
  static rows and the second resolution.

Round 5 (next section) runs the independent per-target work on a worker pool and plans the Position
forest of plain Angle lanes; the rest of A, and B and C, remain.

### Measurement identities

- Baseline: `f67c84e48`, binary `8a95e4a0…`.
- Before and digest reference: `71a68e1d1` built in `.artifacts/tmp/tl639/base`, binary
  `36e0e4b1…` (`.artifacts/tmp/tl639r4/bin/lb-base`).
- Candidate: `71a68e1d1` plus the round-4 working tree, binary `ba02fa6e…`
  (`.artifacts/tmp/tl639r4/bin/lb-final`), `--release --locked --no-default-features`; source
  manifest `.artifacts/tmp/tl639r4/identity/final-source-manifest.json`.
- Evidence: `.artifacts/performance/semantic-output/tl639r4-final-20261003T182820Z/`
  (`legacy/`, `semantic/`, `before-after-typed/`, `repeat-125/`).
  Digest runs `.artifacts/tmp/tl639r4/{d,l}-*.json` and summaries `digest-*.txt` (one per step,
  `s1`-`s15` and `final`), compared by `.artifacts/tmp/tl639r4/cmp.py`; profiles
  `p-final-{hf,s2000}.txt`; allocation counts `allocs.sh`/`allocs-base.sh`.

## Semantic output, round 5: parallel frames, planned forests (TL-639)

Round 5 was asked for two levers, in order: evaluate a frame's independent per-target work on a
fixed worker pool with a deterministic merge, then build per-target compiled plans (design A) for
the Position forest, observer and cohort finish and for component edits (B). The first lever was
built for the final render, the ordinary (non-Position) family groups and the family preparation;
the second for the Position forest of plain Angle lanes, with a verification mode that builds the
general walk beside every plan. Every step was byte-identical to `a16f25bbb` on all 12 digest
workloads, and the final binary at 1, 2, 4 and 18 workers. The legacy tiers gained 13-20 % at
p99. The typed tiers gained 26-38 % at p50 and still miss: stress 2,000 runs at 25 Hz (p99
43 ms), the hard floor at 92 Hz (p99 12.3 ms). The frame is memory-bound: three single-threaded
typed hard floors on the same machine each run 1.45× slower than one, so a worker pool scales it
about 1.4× (hard floor) and 1.6× (stress 2,000), not 6×.

### Gates

Runner, thresholds and references as in round 4; candidate binary `42f92ff0…` with the default
worker count (the available parallelism capped at 8; this host has 18 cores, 6 of them
performance cores). Legacy: five alternated rounds against `f67c84e48`; typed tiers three rounds;
TL-564 one run per configuration. Host: quiet. Pipeline ms, medians.

| Gate (existing threshold) | Round 4 | Round 5 |
| --- | --- | --- |
| Paired p99 regression ≤ max(1 ms, 5 %), stress 2,000 / 60 Hz | pass, +0.23 | **pass**: 5.52 → 4.81 (-0.71) |
| Paired p99 regression, stress 4,000 / 60 Hz | pass, -0.16 | **pass**: 11.06 → 8.80 (-2.26) |
| Paired p99 regression, hard floor 4,148 / 125 Hz | pass, -0.81 | **pass**: 4.13 → 3.37 (-0.76); 125 Hz held 5 of 5 |
| Typed stress 2,000 / 60 Hz: rate held, 0 misses | fail, p99 69.57, 15.7 Hz | **fail**: p50 39.37, p99 43.11, 25.2 Hz |
| Typed stress 4,000 / 60 Hz | fail, p99 140.93, 7.5 Hz | **fail**: p50 87.53, p99 92.38, 11.4 Hz |
| Typed hard floor 4,148 / 125 Hz | fail, p99 17.26, 68.8 Hz | **fail**: p50 10.73, p99 12.32, 91.7 Hz (about 552 misses per round) |
| TL-564 mix, 27 configurations: rate held, 0 misses | 25 pass | 26 pass (see below) |
| No generation change or compile from motion | pass | pass |
| Static bases: 0 Position fits, dependents 6 / 24, unchanged Colour skips solves | pass | pass |
| Readout consumers do not multiply physical solves | pass | pass |

The TL-564 failure is small-subset at 125 Hz output with 30 Hz tracking: four late frames in a run
whose pipeline p99 is 3.6 ms; every TL-564 p99 equals round 4's within 0.5 ms. In the first
campaign of this round (binary `38d3b756…`, the same code without the last fix below) it was
all-points-move 60/60 Hz with one late frame instead. Ten alternated 125 Hz all-points-move repeats
with 30 and 120 Hz tracking (`repeat-125/`): `a16f25bbb` had misses in 3 of 20 runs (5), this
build in 4 of 20 (8), the late frames 8.2-20 ms with the transaction taking all of it; pipeline
p99 medians 3.68 and 3.69 ms. The TL-564 rigs have about a hundred fixtures: their frames stay
below the parallel thresholds except the family groups, and run as before. The isolated late
frames are host scheduling, as in rounds 2-4.

Typed tiers, `a16f25bbb` alternated with this build, three rounds (`before-after-typed/`):

| Profile | `a16f25bbb` p50 / p99 | Round 5 p50 / p99 | Rate | RSS |
| ---: | ---: | ---: | ---: | ---: |
| Typed stress 2,000, 60 Hz | 64.49 / 73.11 | 39.94 / 42.33 | 15.4 → 24.9 Hz | 368 → 391 MB |
| Typed stress 4,000, 60 Hz | 137.02 / 150.90 | 88.30 / 97.09 | 7.2 → 11.3 Hz | 688 → 663 MB |
| Typed hard floor, 125 Hz | 14.31 / 16.62 | 10.53 / 12.18 | 68.5 → 93.7 Hz | 275 → 294 MB |

The p50 falls 38 % (stress 2,000), 36 % (stress 4,000) and 26 % (hard floor). By p99 the typed
tiers are 2.6× (stress 2,000), 5.5× (stress 4,000) and 1.5× (hard floor) from their budgets.

### Worker scaling

`LIGHT_OUTPUT_WORKERS` (a count or `max`) sets the pool; `1` keeps every frame on the output
thread. Final binary, two rounds of 5 s, pipeline p50 / p99 (`workers/`):

| Workers | 1 | 2 | 4 | 6 | 8 (default) | 18 (`max`) |
| --- | --- | --- | --- | --- | --- | --- |
| Typed stress 2,000 | 62.86 / 66.59 | 55.12 / 59.89 | 46.12 / 50.91 | 41.04 / 46.86 | 39.57 / 45.73 | 37.44 / 40.28 |
| Typed hard floor | 14.30 / 17.59 | 13.21 / 16.81 | 11.32 / 13.11 | 10.56 / 12.79 | 10.48 / 13.46 | 10.42 / 13.57 |
| Legacy stress 2,000 | 4.99 / 5.72 | 4.47 / 5.15 | 4.18 / 4.99 | 4.06 / 4.76 | 4.08 / 4.77 | 4.15 / 4.81 |
| Legacy hard floor | 2.86 / 3.51 | 2.66 / 3.20 | 2.48 / 3.30 | 2.52 / 3.29 | 2.54 / 3.28 | 2.56 / 3.19 |

With one worker the frame is the single-threaded loop: against `a16f25bbb` alternated (three
rounds of 4 s, `.artifacts/tmp/tl639r5/w1-ab-final2.txt`, p50) the typed hard floor runs 13.95
against 14.05 ms, typed stress 2,000 62.08 against 63.42, and the legacy tiers are equal (hard
floor 2.80 / 2.86, stress 2,000 4.73 / 4.72). The first final candidate was 0.9 ms slower on the typed hard floor at one worker:
it collected every frame's leftovers for the pool even when there was none, and the per-frame
growth of those collections cost about 0.45 ms with the pool as well. They are collected now
only once a pool takes them.

Why the scaling stops near 1.4-1.7×: the typed frame is memory-bound. Several single-threaded
typed hard floors run as separate processes at once (`concurrent/`, pipeline p50 of each): 1 →
13.81 ms; 2 → 17.77; 3 → 20.04; 6 → 26.17. The legacy hard floor, with a fraction of the
per-target data, does not slow down (2.83, 2.71, 2.39, 2.99). The worker threads of one frame share the same
caches and memory the same way, so each runs its chunk 1.5-2× slower than the single-threaded
loop runs the same groups (2.1 µs against 0.97 µs per static Colour row at 6 workers). The
allocator is not the cause: the benchmark of step 7 under mimalloc (a measurement-only build in
`.artifacts/tmp/tl639r5/mi-bench/`, no repository change) ran stress 2,000 at 43.1 ms with
6 workers against 47.1 ms under the system allocator, and the hard floor unchanged (11.4 against
11.2 ms). What would scale is less memory per
target: fewer and smaller per-frame structures (the compiled plans), not more threads.

### Equivalence

Every step was compared with `a16f25bbb` by `--digest-ticks` on the 12 workloads of round 4
(eight 20-tick workloads, four 30-tick lifecycle runs with fades, FixAT, Freeze and a Preload GO):
0 fixture mismatches, DMX equal on every tick, work counters equal on every tick. The final binary
was also compared with `LIGHT_OUTPUT_WORKERS` 1, 2, 4 and `max` (18), and with
`LIGHT_VERIFY_PLANS=1` (every planned Position forest rebuilt through the general walk and
asserted equal): 12/12 each. The benchmark test `every_output_worker_count_renders_the_single_threaded_digest`
compares full digests (every fixture, universe, point, event and work counter) of typed stress
2,000 and its lifecycle at 1, 2 and 5 workers with plan verification on. Under `cfg(test)` the
parallel thresholds drop to two groups, two samples and two fixtures, so the whole engine and
runtime unit suites (386 and 1,863 tests) also run through the parallel paths.

### What changed

- **Worker pool** (`light_engine::parallel`, `Engine::output_pool`). A `rayon` pool of the
  configured size, owned by the engine and built on first use; `run_ordered` runs a section's
  chunks on it and returns their results in chunk order whatever thread ran them, each thread
  using one scratch slot. A frame never spawns a thread. `drop_later` frees a frame's garbage on a
  pool thread. The worker count comes from `LIGHT_OUTPUT_WORKERS` (desk: `bootstrap.rs`;
  benchmark: the same variable or `--output-workers`); outputs are identical for every count.
  New workspace dependency: `rayon` 1.12 (already in `Cargo.lock` through the Media codec).
- **Final render** (`render_fixtures.rs`). Resolving a fixture reads only the frame's inputs and
  writes its own output. From 512 fixtures the render resolves every fixture's outputs on the
  pool, then writes visualization values, physical readouts and DMX in fixture order on the
  caller, through the same per-fixture control flow, so universe overlaps, the visualization
  map's insert history and the first error are the single-threaded loop's.
- **Ordinary family groups** (`hybrid/parallel_groups.rs`). After the Position batch, each
  non-Position group composes one row of its own target. From 64 groups the cohort gives each
  target (by a fixed hash shard) to one chunk; a chunk composes its groups in cohort order on a
  worker against a fork of the cohort's sources and a private staging of the Color and Focus/Zoom
  lanes; the frame then takes back the keyed changes and walks the groups in order, taking each
  worker group's rows, requirements and source logs at its place and composing itself a Position
  group the batch did not handle, or a group the worker could not finish (a descriptor its lane
  has not compiled; on the first frame after a generation change that is every group), with the
  rest of that chunk.
  - Source forks (`programming_projection/fork.rs`, `memo.rs`). The frame's caches are layered:
    a fork reads the frame's maps as they stood (`frozen`), its earlier groups' entries and its
    current group's; a group the frame reruns drops its layer. Requirements and the first failure
    are ordered logs appended group by group. Every key a group reads or writes is its own
    target's, so a fork answers what the frame's sources would have answered at that point.
  - Source bindings (`dynamic_source_origins/overlay.rs`). Static-baseline binding is one body
    (`bind_static_evidence_in`) over the catalogue or a worker's overlay of it; overlays apply in
    group order. Occurrence ids are random either way.
  - Lanes (`lane/worker.rs`, `family_lanes/parallel.rs`). Observing, adopting and transitioning
    run one body over the lane or a worker's view of it (shared descriptors and committed
    continuity, private staging); `merge_staging` re-runs the token and duplicate checks. Color
    adapters became `Sync`: fitter caches behind `Mutex`es (compiled only on a miss, which a
    worker never performs) and work counters in per-thread shards (`counters.rs`).
  - Kept static rows (`static_rows.rs`) live in 64 shards by target, lent to a worker by moving
    maps.
- **Family preparation** (`preparation/parallel.rs`, `hybrid/parallel_preparation.rs`). From
  256 samples, the controllers (instance, controller, target) are cut by target shard; each chunk
  prepares its controllers in frame order on a worker into an ordered record, against a source
  fork; the frame replays the records controller by controller and prepares itself any
  controller whose fork needed the Position lane (Position Current adoption). The compiled-sample
  cache is read-only to workers and gains their entries afterwards.
- **Planned Position forests (design A, first shape)** (`expression_angles/forest/plain.rs`). A
  controller whose lanes are all plain Angle leaves — authored Angle or Target components, numeric
  Angle programs, Angle Current — has one correlated branch and no transition: its forest is that
  branch, built straight from the leaves with the same leaf calls, Current reads and program
  copies, without importing a tape or walking root sets. Programs with operation origins still
  import into the joint tape (its object table is checked as before) and the branch takes their
  copies from it. `LIGHT_VERIFY_PLANS=1` (or `set_plan_verification`) rebuilds every planned
  forest through the general walk and asserts equality.
  - Leaf-root tape import (`expression_tape.rs`, `import_leaf_roots`). Distinct live leaf roots
    import without the walk's maps into exactly the walk's tape and source keys (test:
    `distinct_leaf_roots_import_exactly_as_the_walk_does`); this also serves every numeric Angle
    sample's validation.
- **Smaller items.** Canonical keys borrowed (`ProgrammingOwner::key_ref`,
  `AttributeKey::{intensity,color}_ref`) so a lookup never touches the shared allocation's count
  that the workers would contend on; last frame's composition samples, the lanes' replaced
  committed entries and the accepted-Color leftovers are freed on the pool (`drop_later`), the
  latter two collected only once a pool takes them.
- Built, measured, kept off: a scalable allocator (above).

### Not built

- **The rest of A: rebinding compiled Position forests, the observer and the cohort finish.**
  The plan above removes the tape import and root-set walk; `from_position_forest` still builds
  the compiled expression and its lineage every frame (5.2 % of stress 2,000 single-threaded), and
  the Position observer's program preparation, composition and cohort finish (about 9 ms of the
  stress 2,000 frame) still run sequentially on the output thread: their cut coordinator and
  shared peers couple targets, and a worker view of the Position lane was not built. Rebinding
  numeric leaves into a kept `CompiledCoupledExpression` needs a slot map from every tape leaf to
  its compiled nodes, endpoint sources and lineage forest nodes; no such map exists, and none was
  derived.
- **B, component edits over a static base.** The Colour rows' composition now runs on the pool;
  its per-frame task list, segments and trace arena are still rebuilt (the compiled-sample cache
  misses whenever a sampled value changes, which is every animated frame).
- **Deferred completion** (about 5.5 ms of stress 2,000) stays sequential: it mutates each Dynamic
  instance's lane transition caches, which are shared by all targets of the instance.
- **The static resolutions** (1.3 ms of the hard floor) and the render's write pass (0.4 ms) stay
  sequential.

### Where the time goes now

CPU samples of the final binary on the output thread (default workers), share of the frame; a
parallel section counts its wall time:

- **Typed stress 2,000 (about 40 ms).** Family preparation section 18 %; ordinary groups section
  10.5 %; the Position groups' program composition on the output thread 9.2 %, their program
  preparation 7.3 % and cohort finish (Position fits, cut coordinator) 7.0 %; deferred completion
  13.8 %; freeing 11 % (mostly last frame's Position programs and samples); pinning 3.4 %, native
  family installation 3.3 %, final render 3.4 %, static resolution 3.1 %. About 37 % of the frame
  is parallel; the Position path and the completion are the sequential bulk.
- **Typed hard floor (about 10.5 ms).** Ordinary groups section (the static Colour rows) 15.4 %;
  the two static resolutions 12.4 %; final render 11.8 % (the parallel resolve and the write
  pass); family preparation section 7.3 %; static target discovery 6.8 %; accepted Colour record
  4.6 %; pinning 4.5 %; row projection 3.9 %; Position cohort finish 2.7 %; scalar projection
  2.5 %; lane acceptance 2.1 %; Current capture 2.0 %.

### Allocation per frame

Counting allocator (`alloc-count/`, as in round 4), objects and bytes per frame:

| Workload | `a16f25bbb` | Round 5, 1 worker | Round 5, 8 workers |
| --- | ---: | ---: | ---: |
| Typed hard floor | 54,497 / 24.5 MB | 54,497 / 24.5 MB | 56,189 / 35.8 MB |
| Typed stress 2,000 | 535,783 / 122.5 MB | 490,401 / 118.7 MB | 500,120 / 144.2 MB |
| Legacy hard floor | 98 / 9.6 MB | 98 / 9.6 MB | 99 / 9.6 MB |
| Legacy stress 2,000 | 2,623 / 22.2 MB | 2,623 / 22.2 MB | 2,624 / 22.2 MB |

The planned forests and the leaf-root import remove 45,000 objects per frame from stress 2,000.
The parallel sections add bytes: each worker's chunk grows its own lists and maps from empty
every frame (about 2.6 MB per hard-floor frame in preparation records, 1.8 MB in composed rows,
1.5 MB in applying the forks' caches); pooling or sizing them gained about 0.1-0.2 ms in a trial
and was not kept for this round.

Round 6 (next section) moves most of the sequential remainder of the typed frame (deferred
completion, Position composition and root fitting, program capture, static target discovery,
frees) onto the pool; the per-target data volume is unchanged.

### Measurement identities

- Baseline: `f67c84e48`, binary `8a95e4a0…`.
- Before and digest reference: `a16f25bbb`, binary `ba02fa6e…` (round 4's final,
  `.artifacts/tmp/tl639r5/bin/lb-base`).
- Candidate: `a16f25bbb` plus the round-5 working tree, binary `42f92ff0…`
  (`.artifacts/tmp/tl639r5/bin/lb-final2`), `--release --locked --no-default-features`; source
  manifest `.artifacts/tmp/tl639r5/identity/final2-source-manifest.json`.
- Evidence: `.artifacts/performance/semantic-output/tl639r5-final2-20261003T225350Z/`
  (`legacy/`, `semantic/`, `before-after-typed/`, `workers/`, `repeat-125/`, `concurrent/`); the
  first candidate's campaign `tl639r5-final-20261003T215101Z/` (binary `38d3b756…`).
  Digest runs `.artifacts/tmp/tl639r5/{d,l}-*.json` and summaries `digest-*.txt` (one per step,
  `p1`-`p12`, `final`, `final2`, `final2-w{1,2,4,max}`, `final2-verify`); profiles
  `p-final2-{s2000,hf}.txt`; allocation counts `allocs-final2.txt`; the mimalloc measurement
  `mi-bench/`.

## Semantic output, round 6: the sequential remainder on the pool (TL-639)

Round 6 was asked to cut memory traffic per target and the sequential remainder of the typed
frame (the Position path and the deferred completion on stress 2,000; the static resolutions and
the render on the hard floor), to extend the compiled per-target plan, and to check the default
worker count for smaller desks. What was built moves most of the output thread's per-target work
onto the worker pool with the round-5 determinism rules, and removes a few whole-frame copies;
the per-target data volume itself did not shrink (allocations per frame are within 1 % of round
5). Every step was byte-identical to `3a19b23f8` on all 12 digest workloads, and the final binaries
also at 1 and 18 workers (step 9 at 1, 2, 4 and 18) and with plan verification. Typed stress 2,000 and 4,000 run 26-27 %
faster at p50 than `3a19b23f8` alternated on the same host; the typed hard floor 6 %. No typed
deadline is met.

### Gates

Runner, thresholds and references as in round 5; campaign binary `b3f2418e…` with the default
worker count (8 on this host). The host was busier than in round 5 (load average 4.7-5.6; the
reference's single-worker typed hard floor measured 18.4 ms during the campaign against 14.3 ms at
the start of the round), so absolute numbers are inflated by roughly a quarter; the alternated
comparisons are like for like. Pipeline ms, medians.

| Gate (existing threshold) | Round 5 | Round 6 |
| --- | --- | --- |
| Paired p99 regression ≤ max(1 ms, 5 %), stress 2,000 / 60 Hz | pass, -0.71 | **pass**: 6.04 → 5.08 (-0.96) |
| Paired p99 regression, stress 4,000 / 60 Hz | pass, -2.26 | **pass**: 12.82 → 10.33 (-2.49) |
| Paired p99 regression, hard floor 4,148 / 125 Hz | pass, -0.76 | **pass**: 5.24 → 3.65 (-1.59); 125 Hz held 5 of 5 |
| Typed stress 2,000 / 60 Hz: rate held, 0 misses | fail, p99 43.11, 25.2 Hz | **fail**: p50 33.42, p99 36.29, 29.7 Hz |
| Typed stress 4,000 / 60 Hz | fail, p99 92.38, 11.4 Hz | **fail**: p50 71.11, p99 100.65, 13.8 Hz |
| Typed hard floor 4,148 / 125 Hz | fail, p99 12.32, 91.7 Hz | **fail**: p50 12.35, p99 13.55, 80.5 Hz (about 490 misses per round) |
| TL-564 mix, 27 configurations: rate held, 0 misses | 26 pass | 25 pass (see below) |
| No generation change or compile from motion | pass | pass |
| Static bases: 0 Position fits, dependents 6 / 24, unchanged Colour skips solves | pass | pass |
| Readout consumers do not multiply physical solves | pass | pass |

The typed rows above are the capacity suite on the busy host. Alternated against `3a19b23f8`
(three rounds, `before-after-typed/`):

| Profile | `3a19b23f8` p50 / p99 | Round 6 p50 / p99 | Rate | RSS |
| ---: | ---: | ---: | ---: | ---: |
| Typed stress 2,000, 60 Hz | 45.08 / 47.55 | 33.29 / 35.74 | 22.1 → 29.8 Hz | 390 → 400 MB |
| Typed stress 4,000, 60 Hz | 97.68 / 102.78 | 71.34 / 79.14 | 10.1 → 13.9 Hz | 670 → 760 MB |
| Typed hard floor, 125 Hz | 12.85 / 14.28 | 12.06 / 14.15 | 76.9 → 81.9 Hz | 287 → 296 MB |

At the start of the round, on a quiet host, the reference ran typed stress 2,000 at 39.2 / 41.7
ms and the typed hard floor at 10.4 / 12.0 ms (8 workers); scaled by the alternated ratios the
round-6 binary would be near 29 ms and 9.7 ms there. By p99 the typed tiers remain about 2.1×
(stress 2,000) and 1.8× (hard floor, busy host) from their budgets.

TL-564: the two failures are static-points with 60 Hz tracking and small-subset with 120 Hz
tracking, both at 125 Hz output, with 3 and 1 late frames in runs whose pipeline p99 is 2.7 and
3.6 ms. Ten alternated repeats of each and the twenty all-points-move repeats at 125 Hz
(`repeat-125-fails/`, `repeat-125/`): `3a19b23f8` had late frames in 3 of 40 runs (4 frames),
round 6 in 6 of 40 (8 frames); pipeline p99 medians agree within 0.1 ms. Every slow frame the benchmark
printed after warm-up (`LIGHT_BENCHMARK_SLOW_FRAME_MS=8`) is 8-17 ms of transaction with a
0.7-8 ms tracking component, the same signature in both binaries; as in rounds 2-5 they are attributed to host scheduling. The TL-564 rigs have about a
hundred fixtures; of this round's sections only the Position composition (64 groups) and root
fitting (64 roots) can engage there.

### Worker scaling and the default for smaller desks

Final binary, two rounds of 5 s on the busy host, pipeline p50 / p99 (`workers/`):

| Workers | 1 | 2 | 4 | 6 | 8 (default) | 18 (`max`) |
| --- | --- | --- | --- | --- | --- | --- |
| Typed stress 2,000 | 68.09 / 72.06 | 57.65 / 63.42 | 42.48 / 46.41 | 36.55 / 39.48 | 33.25 / 35.58 | 29.99 / 42.00 |
| Typed hard floor | 18.13 / 20.61 | 15.25 / 17.10 | 12.81 / 14.83 | 11.88 / 13.78 | 12.03 / 13.43 | 11.98 / 13.42 |
| Legacy stress 2,000 | 5.54 / 6.31 | 4.76 / 5.37 | 4.31 / 4.92 | 4.22 / 5.13 | 4.21 / 4.86 | 4.33 / 4.99 |
| Legacy hard floor | 3.10 / 4.16 | 2.85 / 3.59 | 2.80 / 3.43 | 2.72 / 3.58 | 2.61 / 3.39 | 2.85 / 3.43 |

Single-worker frames are not slower than `3a19b23f8` (alternated, three rounds:
typed hard floor 18.43 → 18.12 ms, typed stress 2,000 69.12 → 68.05 ms; `w1-ab-*.txt`).

The default stays `min(available_parallelism, 8)`. On this host four workers already give 87 % of
the eight-worker hard-floor gain and 74 % of the stress-2,000 gain, and the legacy tiers saturate
at four. On a four-core desk the default is four workers: the output thread sleeps on the
section's latch while they run, so a section uses at most the four cores, and everything outside
the sections stays on the output thread. That machine's own memory bandwidth and core mix decide
the absolute numbers; this 18-core host cannot emulate it (macOS has no core affinity). If a
four-core desk's interface stutters during typed output, `LIGHT_OUTPUT_WORKERS=3` is the
setting to try; outputs are identical for every count.

Memory boundedness, re-measured (`concurrent/`, single-worker processes side by side, p50 of
each): typed hard floor 18.09 → 20.18 → 22.25 → 28.10 ms for 1, 2, 3 and 6 processes; legacy
3.04 → 3.25 → 3.51 → 4.06 ms (the busy host slows the legacy tier too this time).

### Equivalence

Every step was compared with `3a19b23f8` by `--digest-ticks` on the 12 workloads of rounds 4-5:
0 fixture mismatches, DMX equal on every tick, work counters equal on every tick. Step 5 and both
final binaries were also compared at `LIGHT_OUTPUT_WORKERS` 1 and `max` (18), step 9 at 1, 2, 4
and 18, and all three with `LIGHT_VERIFY_PLANS=1`: 12/12 each. Under `cfg(test)` every new section's threshold is two items,
so the engine, dynamics and runtime unit suites run them.

### What changed

- **Deferred completion on the pool** (`light_dynamics::runtime::sampling::staged::parallel`).
  Evaluating a typed lane reads its compiled lane, its controller's pins and Current of its own
  target; the only write, a keyframe transition for the lane's cache, is now returned
  (`CompiledProgrammingLane::sample_with_cache`) and kept by the frame in order
  (`keep_transition`), so a worker never touches the lane. Lanes are cut by target shard; each
  chunk evaluates in frame order against a fork of the sources (the round-5 forks, one group per
  lane); the frame resolves every plan exactly as before, taking each evaluation from its chunk's
  record and appending the fork's logs at that place, or evaluating a lane itself (and every later
  one of its chunk) where the fork needed the Position lane.
- **Position composition on the pool** (`physical_adapter::position::frame_worker`). One body
  composes a Position program (and observes its rows) over the frame's observer or over a
  worker's read-only view of it: captured programs, Current cohorts, the lane's descriptors and
  committed continuity. The ordinary-groups section now composes the Position groups the batch
  does not handle on the worker that owns their target; each worker records the pending cohort
  members and the frame takes them in group order. `PositionAdapter` became `Sync` (instance cache
  and counters behind `Mutex`es, tracking behind a read/write lock, a Current cohort's lazy fit
  behind a `Mutex`, so it still runs once); the thread audit records it.
- **Root fitting on the pool** (`PositionAdapter::resolve_cohort_programs_on`). A physical root's
  fit reads its own requests, instances and accepted continuity and writes only its members'
  resolutions; from 64 roots they fit on the pool and the frame takes results, the first error and
  the work counters in root order.
- **Program capture on the pool** (`frame_observer::capture_programs`). Each Position program's
  registry is built on the pool from one shared sample list (the requested program and the
  registry held two copies before); capture identities are drawn in program order first.
- **Static target discovery on the pool** (`lane::static_targets`). The Position, Colour and Zoom
  scans read only the baseline and compiled descriptors; chunks of fixtures run on the pool and the
  frame applies the duplicate and error rules in order. A descriptor not yet compiled for this
  generation sends the scan back to the frame.
- **Less copying.** The frame's completed samples are moved into the published sample list, not
  cloned (`CompletedDynamicSamples::take_samples`); emission moves each pinned expression instead
  of cloning it and keeps an already shallow one (`into_shallow`); retained sample maps hash with
  Fx; Position's first-pending lookup is an index, not a scan per row.
- **Frees and sorts on the pool.** Last frame's unused compiled samples
  (`DynamicFamilyPreparationScratch::take_retired`), the replaced visualization frame and the
  Position observer's frame state are freed by `drop_later`; the controller and group sorts run as
  parallel sorts of distinct keys (`light_engine::parallel::sort_unstable_by_key`).
- **Lanes are lent per section.** Preparation and completion now borrow the lanes only for each
  parallel section, so a controller or lane the frame prepares itself after a stop adopts through
  unborrowed lanes. In round 5 the lanes stayed lent for the frame's replay walk as well, where a
  frame-side Colour or Optics adoption after a stopped chunk would have found its lane borrowed.

### Not built

- **Smaller per-target data (item 1-2 of the brief).** Measured: `DynamicRuntimeSample` is 240
  bytes, `FamilyCompositionSample` 192, `DynamicValueAddress` 160 (a Direct Colour address holds
  its native identity inline, two `String`s), `DynamicSampleExpression` 136,
  `CompiledCoupledExpression` 256. Boxing the native identity touches about 70 sites (most of
  them tests) and does not change the stress workloads, which hold no Direct address; it was not
  done. Allocations per frame are unchanged: the largest per-target clusters are the Position
  forest compile in preparation (`from_position_forest` and its node, metadata and endpoint
  lists, about 50,000 objects per stress frame), the captured Position registries (about 14,000),
  and the Colour composition's traces and adapter scratch.
- **Extending the compiled plan (item 4).** Target leaves are already planned (round 5's `plain`
  forest covers authored Target components); the remaining cost of a planned forest is
  `from_position_forest`, which rebuilds the compiled graph from the branch every frame. Reusing a
  kept graph needs the leaf → compiled-node map round 5 identified; it was not derived. Component
  edits over a static base (design B) were not started. No plan was added, so
  `LIGHT_VERIFY_PLANS` covers the same forests as in round 5.
- **Parallel emission and pinning.** Both mutate one Dynamic instance per plan, but through the
  shared output-frame undo journal (`OutputFrameUndo`); splitting it into per-instance handles
  was not done. Together about 4 ms of the stress-2,000 frame.
- **Parallel static resolution.** The engine's resolver records slots in first-write order, which
  later readers observe; a sharded offer pass would have to rebuild that order. Not done.
- **Replaying unchanged static rows.** The hard floor composes, observes, binds and stages every
  static Colour row every frame (about 14 ms of worker CPU per frame); replaying an unchanged
  group's whole output needs proof that its sources, bindings and lane state are unchanged. Not
  done.

### Where the time goes now

`sample` of the final binary, output thread by exclusive section, and the pool's CPU per section
(busy host, 8 workers):

- **Typed stress 2,000 (about 33 ms).** Parallel sections about 13.8 ms of wall time for about
  98 ms of worker CPU per frame: ordinary and Position groups 41 ms, preparation 28 ms,
  completion 11 ms, frees 8.6 ms (in the background), render resolve 4.4 ms, root fits 4.0 ms.
  The output thread's own work is about 19.6 ms: completion's ordered apply and emission 2.8 ms,
  frees 2.3, frame glue 2.0, pinning 1.4, native installation 1.4, preparation's ordered merge
  1.2, static resolution 1.1, program capture remainder (tracking, roots) 1.1, render write pass
  0.95, input assembly 0.86, source binding 0.85, fixed bases 0.67, Position staging 0.6, and
  smaller items. The sequential part alone exceeds the 16.7 ms budget on this host; the parallel
  part is memory-bound (a worker's chunk runs 1.5-3× slower than the same items single-threaded).
- **Typed hard floor (about 12 ms on the busy host).** Parallel sections about 3.5 ms for about
  23 ms of worker CPU (static Colour rows 14 ms, preparation 4.4, render resolve 3.0, static
  scans 1.3). The output thread's own work is about 8.5 ms: static resolution 1.3, render write
  pass 1.0, frame glue 0.8, frees 0.7, pinning 0.5, accepted Colour record 0.5, row projection
  0.5, lane acceptance 0.4, and smaller items.

### Allocation per frame

| Workload | `3a19b23f8` 1 / 8 workers | Round 6, 1 worker | Round 6, 8 workers |
| --- | ---: | ---: | ---: |
| Typed hard floor | 54,497 / 24.5 MB; 56,189 / 35.8 MB | 54,405 / 24.5 MB | 56,645 / 37.0 MB |
| Typed stress 2,000 | 490,401 / 118.7 MB; 500,120 / 144.2 MB | 484,887 / 118.4 MB | 503,215 / 164.4 MB |
| Legacy hard floor | 98 / 9.6 MB; 99 / 9.6 MB | 98 / 9.6 MB | 99 / 9.6 MB |
| Legacy stress 2,000 | 2,623 / 22.2 MB; 2,624 / 22.2 MB | 2,623 / 22.2 MB | 2,624 / 22.2 MB |

The new parallel sections add their ordered records (completion evaluations, Position pending
members) to the eight-worker bytes.

Round 7 (next section) moves pinning, the ordered completion, the static resolution's offers and
the native installation onto the pool, and removes some per-frame copies; the typed deadlines are
still missed.

### Measurement identities

- Baseline: `f67c84e48`, binary `8a95e4a0…`.
- Before and digest reference: `3a19b23f8`, binary `42f92ff0…` (round 5's final,
  `.artifacts/tmp/tl639r6/bin/lb-ref`).
- Campaign candidate: `3a19b23f8` plus the round-6 working tree before the per-section lane
  lending, binary `b3f2418e…` (`.artifacts/tmp/tl639r6/bin/lb-final`); source manifest
  `.artifacts/tmp/tl639r6/identity/final-source-manifest.json`.
- Final tree: binary `30d8baf6…` (`lb-final2`), source manifest `final2-source-manifest.json`;
  digest-identical to `3a19b23f8` at 1, 8 and 18 workers and with plan verification, and equal
  to the campaign binary in alternated runs (stress 2,000 33.12 / 33.29 ms, hard floor 11.60 /
  11.70 ms p50).
- Evidence: `.artifacts/performance/semantic-output/tl639r6-final-20261004T021437Z/`
  (`legacy/`, `semantic/`, `before-after-typed/`, `workers/`, `repeat-125/`,
  `repeat-125-fails/`, `concurrent/`). Digest runs and summaries
  `.artifacts/tmp/tl639r6/{d,l}-*.json`, `digest-*.txt` (`s1`-`s9`, `final`, `final2`, worker
  counts, `-verify`); profiles `p-*-{s2000,hf}.txt` with `sections.py`, `mainthread.py`,
  `workerthreads.py`; allocation counts `allocs-final.txt` and allocation sites
  `allocsites-*.txt` (`alloc-count/`, raw return addresses resolved at exit).

## Semantic output, round 7: the remaining sequential frame on the pool (TL-639)

Round 7 was asked to compile typed semantic frames once per patch so the typed tiers meet their
deadlines, starting from round 6's roadmap: a kept compiled Position graph, parallel emission and
pinning, replay of unchanged static Colour rows, a parallel static resolution, and memory. Measured
by payoff, what was built is mostly the sequential remainder: pinning and the ordered completion
per Dynamic instance on the pool (with the output-frame undo journal lent per instance), the static
resolution's offers by slot range, the native installation by physical instance, and a set of
copies and contended counts removed. The kept Position graph and the static-row replay were not
built (reasons below). Every step was byte-identical to the round's start head `dc289b3a7` on all
12 digest workloads; the final binary also at 1, 2, 4 and 18 workers and with plan verification.
Alternated against `dc289b3a7`, typed stress 2,000 runs 14 % faster at p50, stress 4,000 17 % and
the typed hard floor 12 %; the hard floor's resident memory falls by a fifth. No typed deadline is
met: the hard floor is about 1.1× its 8 ms budget at p50, stress 2,000 1.6×.

The reference is the round's start head, not round 6's commit: `dc289b3a7` is round 6
(`54d285e2e`) plus the product work merged since (typed family lanes, Direct derived models, Freeze
and Masters changes), which moved the typed frame; on a quiet host at the start of the round it ran
typed stress 2,000 at 29.6 / 31.5 ms and the typed hard floor at 10.0 / 10.7 ms (p50 / p99).

### Gates

Runner, thresholds and references as in round 6; campaign binary `b374c295…` with the default worker
count (8). The host was shared during the campaign: the user's ToskLight desk (a debug
`light-headless`, about half a core) ran from the same worktree, another worktree compiled the
desktop app shortly before, and the load average rose from 5 to 28 near the end (see
`host-*.txt`); absolute numbers are inflated, the alternated comparisons are like for like.
Pipeline ms, medians.

| Gate (existing threshold) | Round 6 | Round 7 |
| --- | --- | --- |
| Paired p99 regression ≤ max(1 ms, 5 %), stress 2,000 / 60 Hz | pass, -0.96 | **pass**: 6.23 → 5.68 (-0.55) |
| Paired p99 regression, stress 4,000 / 60 Hz | pass, -2.49 | **pass**: 11.75 → 9.17 (-2.58) |
| Paired p99 regression, hard floor 4,148 / 125 Hz | pass, -1.59 | **pass**: 5.04 → 3.71 (-1.34); 125 Hz held 5 of 5 |
| Typed stress 2,000 / 60 Hz: rate held, 0 misses | fail, p99 36.29, 29.7 Hz | **fail**: p50 26.68, p99 28.67, 37.3 Hz |
| Typed stress 4,000 / 60 Hz | fail, p99 100.65, 13.8 Hz | **fail**: p50 55.54, p99 57.54, 18.0 Hz |
| Typed hard floor 4,148 / 125 Hz | fail, p99 13.55, 80.5 Hz | **fail**: p50 9.08, p99 10.22, 108.7 Hz (about 650 misses per round) |
| TL-564 mix, 27 configurations: rate held, 0 misses | 25 pass | 26 pass (see below) |
| No generation change or compile from motion | pass | pass |
| Static bases: 0 Position fits, dependents 6 / 24, unchanged Colour skips solves | pass | pass |
| Readout consumers do not multiply physical solves | pass | pass |

The first campaign of this round (binary `a987f795…`, the final code without the last fix below, on
a quieter host, `tl639r7-final-20261006T200405Z`) measured the same gates: paired p99 5.63 → 5.24,
10.20 → 7.98 and 3.98 → 3.52; typed p50 / p99 25.25 / 26.50, 53.63 / 54.87 and 8.75 / 9.05 ms; TL-564
27 of 27.

TL-564: the one failure is static-points with 120 Hz tracking at 60 Hz output, one late frame in a
run whose pipeline p99 is 6.2 ms, logged while the load average was rising. Ten alternated repeats of
that configuration on a calmer host (`repeat-fail/`): `dc289b3a7` and round 7 had no late frame in
any of their 20 runs (pipeline p99 medians 5.83 and 5.69 ms). The twenty 125 Hz all-points-move
repeats (`repeat-125/`) had no late frame either. As in rounds 2-6 the isolated late frame is
attributed to host scheduling.

Typed tiers, `dc289b3a7` alternated with this build, three rounds (`before-after-typed/`):

| Profile | `dc289b3a7` p50 / p99 | Round 7 p50 / p99 | Rate | RSS |
| ---: | ---: | ---: | ---: | ---: |
| Typed stress 2,000, 60 Hz | 31.84 / 33.33 | 27.35 / 28.39 | 31.2 → 36.4 Hz | 413 → 425 MB |
| Typed stress 4,000, 60 Hz | 67.98 / 69.79 | 56.73 / 58.22 | 14.6 → 17.5 Hz | 757 → 793 MB |
| Typed hard floor, 125 Hz | 10.85 / 11.81 | 9.52 / 10.40 | 91.4 → 104.3 Hz | 308 → 242 MB |

In the quieter first campaign: 29.04 / 29.91 → 25.60 / 26.54, 63.65 / 68.89 → 54.13 / 55.48 and
9.80 / 10.29 → 8.67 / 8.94 ms. By p99 on the quieter host the typed tiers remain about 1.6×
(stress 2,000) and 1.1× (hard floor) from their budgets.

### Worker scaling

Final binary, three rounds of 5 s, pipeline p50 / p99 (`workers/`, busy host):

| Workers | 1 | 2 | 4 | 6 | 8 (default) | 18 (`max`) |
| --- | --- | --- | --- | --- | --- | --- |
| Typed stress 2,000 | 64.96 / 67.92 | 53.44 / 55.03 | 36.34 / 38.84 | 29.79 / 32.30 | 27.34 / 28.67 | 25.03 / 31.61 |
| Typed hard floor | 15.69 / 17.08 | 13.28 / 15.54 | 10.27 / 12.69 | 9.72 / 11.11 | 9.56 / 10.59 | 10.36 / 11.72 |
| Legacy stress 2,000 | 5.67 / 6.57 | 5.05 / 5.72 | 4.86 / 5.53 | 4.75 / 5.43 | 4.43 / 5.62 | 4.82 / 5.33 |
| Legacy hard floor | 3.34 / 4.08 | 3.14 / 3.72 | 2.98 / 3.74 | 3.06 / 3.79 | 2.92 / 3.66 | 3.11 / 3.63 |

The hard floor is now slower with 18 workers than with 8: its new sections are short, and handing
them to the efficiency cores costs more than they save. The default stays `min(available
parallelism, 8)`. Single-worker frames are not slower than `dc289b3a7` (alternated, three rounds of
4 s: typed hard floor 14.47 → 14.03 ms, typed stress 2,000 62.56 → 60.80 ms; `w1-ab-*.txt`). Memory
boundedness is unchanged (first campaign, single-worker processes side by side, p50 of each: typed
hard floor 13.80 → 17.22 → 19.49 → 26.20 ms for 1, 2, 3 and 6 processes; the second campaign's
`concurrent/` ran during the load spike and is not used).

### Equivalence

Every step was compared with `dc289b3a7` by `--digest-ticks` on the 12 workloads of rounds 4-6:
0 fixture mismatches, DMX equal on every tick, work counters equal on every tick (the digests hash
every accepted Colour head and output too). The final binary was also compared at
`LIGHT_OUTPUT_WORKERS` 1, 2, 4 and `max` (18) and with `LIGHT_VERIFY_PLANS=1`: 12/12 each. New
focused tests compare each parallel path with the single-threaded one under `cfg(test)` thresholds
of two items:

- `instances_pinned_and_completed_on_workers_equal_the_single_threaded_frame` (light-dynamics):
  three instances (Keyframes with Current and a typed lane, a Random lane, a JoinSyncNow instance
  paused and resumed) pinned and completed on threads started in reverse order, frames with and
  without typed lanes, every fifth frame failing after completion: samples in order and the runtime
  snapshot equal the single-threaded runtime every frame, and a failed frame restores the same
  runtime.
- `offers_by_slot_range_resolve_the_single_threaded_fill` (light-engine): winners, first-write
  order, traced origins and evidence, and the overflow, with and without tracing, also from a pool
  thread; `traced_offers_keep_an_equal_origin_and_evidence_and_build_only_for_a_winner` and
  `a_held_origin_is_kept_only_while_it_describes_the_offer` for the kept origins.
- `an_installation_on_the_pool_is_the_single_pass_installation` (light-engine): grouped,
  interleaved, duplicated, foreign, incomplete, out-of-range and conflicting write collections give
  the same installation (map order included) or the same first rejection.
- `legacy_leaves_pass_through_as_the_owner_split_would_produce_them` (light-dynamics) and an
  extended `the_accepted_frame_report_reads_published_color_results_and_invents_no_white`
  (consecutive accepted frames of a held look share their recorded lists).

Nothing in this round is kept across frames that a patch change, show switch, Freeze, Preload branch
or source edit would have to invalidate: the kept values are an origin or evidence a slot already
holds (compared by content or identity at every offer), the previous accepted frame's equal lists
(compared element by element at every record), and buffers.

### What changed

Payoffs are alternated p50 medians against the previous step (three or four rounds of 4 s).

- **Pinning and the ordered completion per instance on the pool** (`4b4e9ac97`, `878229e91`;
  `runtime/sampling/staged/instances.rs`, `transaction.rs`). Pinning an instance and completing it
  (keyframe caches, held and last samples, emission) read only frame-immutable sources and write only
  that instance. The output-frame undo journal is lent per instance (`OutputFrameUndo::lend`,
  `InstanceUndo`, both behind the `SamplingJournal` trait), the instances run on the pool
  (`InstanceWorkers`, implemented by the engine's `OutputPool`), and journals, samples and
  requirements are taken back in instance order. Every instance's frame is prepared in order before
  any is pinned. The completion goes parallel only when every typed lane was evaluated by a worker
  (or there is none), so no lane is ever evaluated against the frame's sources off the frame's
  thread. Pinning needs shareable sources (`sample_all_programming_staged_shared`). Stress 2,000
  29.70 → 27.99 ms, hard floor 9.90 → 9.63 ms.
- **Traced origins built only for a winner** (`f33bbc4af`; `contribution/offered_origin.rs`,
  `frame_state.rs`). A tracing resolver allocated every offer's origin eagerly and replaced every
  slot's origin and evidence each frame; a winning offer now keeps the origin (by content) and the
  evidence (by allocation) its slot already holds. With the retired preparation output below: hard
  floor 9.58 → 9.34 ms, stress 2,000 27.95 → 27.29 ms.
- **Preparation output retired whole; legacy leaves passed through** (`20fa8edbc`). Last frame's
  groups and legacy fragments move out with their storage and are freed on the pool; a new group
  takes its controller's samples with their buffer; a legacy scalar leaf becomes its legacy fragment
  without the owner split's four copies (the hard floor's Intensity Dynamics). Hard floor 9.32 →
  8.99 ms.
- **The resolution's offers on the pool by slot range** (`8d7c95ab0`, `1b20759a3`;
  `contribution/parallel_offers.rs`, `frame_state/shards.rs`). With the Programmer's kept winners,
  the Playback contributions, those winners and the sampled values are offered in one section: the
  frame finds every offer's slot in order (unnumbered pairs still reach the overflow map at once),
  groups the numbered offers by contiguous slot range, and each range takes its offers in their
  original order on a pool thread; the ranges' first writes are merged back by offer position, so
  the fill's first-write order (`FrameState::occupied`) is the single-threaded one. A changed
  Programmer's owned winners are offered in turn. A resolution on a pool thread offers in turn: a
  Freeze's captured values are observed lazily, so with instances pinned on the pool a resolution
  can run inside a parallel section, and a nested section could take up another item of that section
  that waits on this resolution. The ranges and merge buffers are kept in the engine. Stress 2,000
  27.16 → 26.34 ms, hard floor 8.91 → 8.71 ms, legacy tiers unchanged.
- **A fresh native installation on the pool** (`7e6b69c19`; `native_family_projection/parallel.rs`).
  Validation and placement of an animated frame's fitted native writes depend only on a write, its
  run's owner and earlier writes of the same physical instance, so the writes are cut by instance;
  the earliest rejection over all chunks is the single pass's, and without one the rows, targets and
  counts are merged in the single pass's creation order, so map order and the completeness check
  are unchanged. The replaced installation is freed on the pool. Stress 2,000 26.58 → 26.38 ms.
- **Completion records sized once** (`d2d30cc18`). The typed-lane records were regrown, and each
  lane's compiled program found by a SipHash lookup per target. Stress 2,000 26.18 → 25.73 ms.
- **Position forest graph sized once** (`05954c891`). The compiled graph's lists grew from one entry;
  `from_position_forest` takes 15 % less worker CPU, below the noise in wall time.
- **Borrowed canonical keys on the workers' paths** (`6465bbca6`). Static-row binding and baseline
  guards, Angle validation, numeric Angle Current, emission's address check and pinning's Current
  reads cloned the process-wide canonical owner or lane keys; with eight workers every clone and
  drop contended on one count. Stress 2,000 25.84 → 25.55 ms, hard floor 8.73 → 8.62 ms.
- **Memory: the accepted Colour ring shares a held look** (`41d86c04e`). The ring of 32 accepted
  frames kept a full copy of each frame's head list and every output's write list (about 80 MB of the
  hard floor's heap); a frame shares the previous frame's head list, and an output the previous
  frame's write list of the same target, while they are equal. Typed hard floor RSS 314 → 228 MB
  (two runs each), timing unchanged. Compiled Colour fitters were already shared between identical
  instances (`CompiledModelInterner`, an earlier TL-639 change), so item 5's first candidate was not the cause.
- Built, measured, reverted: the cohort's per-group scans (`programs`, protected Current, eligible
  Position groups) on the pool: stress 2,000 -0.18 ms, hard floor +0.16 ms.

### Not built

- **A kept compiled Position graph (item 1).** The Position forest bundle is now 44 % of the
  preparation section's worker CPU on stress 2,000 (`from_position_forest` 21 %, the plain forest
  16 %, of which the joint tape import of numeric programs with operation origins is a third). Every
  animated frame changes the values inside the graph (the numeric programs, the captured Current,
  the Whole leaf, the lineage tape), so reuse needs the leaf → compiled-node map to rebind them into
  a kept graph, plus a proof that the tape import's operation-object check holds for the rebound
  programs; neither was derived. The lineage fields `compiled_nodes` and `tape_to_compiled` are never
  read outside tests, so a lazily built lineage is the cheaper first step. Expected payoff at most
  about 1 ms of stress 2,000's preparation section.
- **Replaying unchanged static Colour rows (item 3).** The hard floor's static Colour rows are its
  largest section (1.7 ms wall, 7 ms of worker CPU per frame). A kept row already replays its
  composition, observation fields and fit; what each row still pays is the proof itself: the key
  channels read from the scalar token (the fit's replay key), the static-evidence binding that keeps
  the source record alive, the baseline guard and the staging. A cheaper frame-level proof would
  need the static token's Colour channels to be known unchanged, which needs a Playback "contributions
  unchanged" signal (the Playback engine rebuilds its contributions every frame) and the scalar
  samples' owner footprint; neither exists.
- **Item 4** was built for the offers; the resolution's remainder (Programmer state capture, the
  Playback list) stays on the frame's thread. Parallel offers help stress 2,000 more than the hard
  floor, whose 17,000 offers per resolution are too few to pay for a section.
- **Legacy stress 4,000 physical projection (item 5)** was not needed: every legacy gate improved.

### Where the time goes now

`sample` of the final binary on a quiet host (8 workers), output thread by section (a parallel
section counts its wall time), and the pool's CPU per frame:

- **Typed stress 2,000 (about 25.6 ms).** Ordinary and Position groups 5.4 ms (26 ms of worker CPU),
  preparation 4.5 (17 ms; the Position forest bundle 44 % of it), completion 2.5 (its ordered merge
  of fork logs and the instance section), Position program preparation 1.5, render 1.4, cohort glue
  1.3 (per-group static lookups and guards), Position root fits 1.2, native installation 1.1 (the
  write list's construction about a quarter of it), frees 0.9, input assembly 0.8, the two static
  resolutions 1.3, source binding 0.7, accepted Colour record 0.5, static scans 0.4. Frees on the pool
  6 ms of worker CPU.
- **Typed hard floor (about 8.7 ms).** Static Colour rows 1.7 ms (7 ms of worker CPU), render 1.2
  (the write pass 0.7: DMX encoding hashes its universe maps with SipHash, 0.25 ms; the queued family
  projections 0.25), static resolutions 0.9, preparation 0.5, accepted Colour record 0.5, cohort glue
  0.5, playback resolution 0.4, static scans 0.3, lane acceptance 0.3, row projection 0.3, pinning
  0.3, Position programs 0.3, native installation 0.3, frees 0.2.

### Allocation per frame

Counting allocator (`alloc-count/`, and `alloc-count-ref/` built from an archive of `dc289b3a7`),
objects and bytes per frame:

| Workload | `dc289b3a7` 1 / 8 workers | Round 7, 1 worker | Round 7, 8 workers |
| --- | ---: | ---: | ---: |
| Typed hard floor | 54,404 / 22.8 MB; 56,647 / 34.7 MB | 45,074 / 22.7 MB | 47,356 / 35.9 MB |
| Typed stress 2,000 | 484,291 / 117.8 MB; 503,441 / 163.8 MB | 479,218 / 120.3 MB | 499,150 / 177.8 MB |
| Legacy hard floor | 97 / 7.7 MB; 98 / 7.7 MB | 97 / 7.7 MB | 106 / 8.2 MB |
| Legacy stress 2,000 | 2,624 / 20.6 MB; 2,625 / 20.6 MB | 2,624 / 20.6 MB | 2,634 / 21.5 MB |

The hard floor allocates 17 % fewer objects (the legacy leaves' copies). The eight-worker bytes grow
with the new sections' per-instance and per-chunk records (stress 2,000 +14 MB per frame: the
instances' sample and requirement runs, the native installation's chunk maps, the retired
preparation buffers); the object counts do not. Resident memory: typed hard floor 289-308 → 242-260 MB
(the two campaigns), stress 2,000 and 4,000 unchanged within run-to-run spread (395-413 → 419-425 MB,
737-757 → 793-828 MB; single runs vary by ±40 MB).

### Measurement identities

- Baseline (legacy gates): `f67c84e48`, binary `8a95e4a0…`.
- Before and digest reference: the round's start head `dc289b3a7`, binary `eae82653…`
  (`.artifacts/tmp/tl639r7/bin/lb-ref`).
- Final: `1b20759a3`, binary `b374c295…` (`lb-final4`, campaign copy `lb-final`),
  `--release --locked --no-default-features`; source manifest
  `.artifacts/tmp/tl639r7/identity/final4-source-manifest.json`.
- First campaign: `6465bbca6`, binary `a987f795…` (`lb-final1`, manifest
  `final1-source-manifest.json`).
- Evidence: `.artifacts/performance/semantic-output/tl639r7-final-20261006T221603Z/` (`legacy/`,
  `semantic/`, `before-after-typed/`, `workers/`, `repeat-125/`, `repeat-fail/`) and the first
  campaign `tl639r7-final-20261006T200405Z/` (also `workers3/`, `concurrent/`). Digest runs and
  summaries `.artifacts/tmp/tl639r7/{d,l}-*.json`, `digest-*.txt` (per step `a1`-`g1`, `final`-`final4`,
  worker counts, `-verify`); profiles `p-*-{s2000,hf}.txt` with `sections-wall.py`, `secitems.py`,
  `callers.py`; allocation counts `allocs-final*.txt`; alternated single-worker runs `w1-ab-*.txt`.

## Semantic output, round 8: the owner's targets at 8, 16 and 24 universes (TL-639)

Round 8 started against round 7's tiers (typed stress 2,000 and 4,000, the 32-universe typed hard
floor at 125 Hz). During the round the owner replaced those deadlines: the ceiling is 24 universes
× 512 = 12,288 parameters, nothing beyond it is optimised or benchmarked, and the targets are the
four rows below. None of round 7's typed tiers is inside the ceiling (stress 2,000 needs 74
universes, the sustained hard floor 32), so the round added typed workloads of the target sizes and
measured them. Every target row is met on this machine with a p99 at most 0.43 of its budget, and
the entry-level estimate holds 40 Hz at 8 universes when the frame runs on two or more threads; the
binding case is the typed stress mix on one thread. No engine code changed: with the targets met,
none of the profiled candidates (below) had a payoff that justified a cross-frame cache or a new
proof. The two commits are benchmark-only, and the final binary is byte-identical to the round's
start head `892b0ee29` on all digest workloads.

### Workloads at the target sizes

- **Sustained show on 8, 16 and 24 universes** (`--profile hard-floor --sustained-show
  --universes 8|16|24`, `f30269cad`, `15a0003f5`): the 32-universe mix with every manufacturer
  quantity scaled exactly by `universes / 32` and the RGB PARs filling each universe: 1,037, 2,074
  and 3,111 fixtures on 4,096, 8,192 and 12,288 slots. The typed variant (`--semantic`) is the
  hard floor's: a static semantic Colour and Position base on every fixture and Intensity Dynamics
  on every target. 32 universes is unchanged.
- **Headless stress on 200, 400 and 650 fixtures** (`--headless-stress-fixtures`): the stress mix
  (20 Dynamics, typed Colour recipe and Angle lanes on 54 % of the fixtures) on 8, 15 and 24
  universes (3,772, 7,544 and 12,259 slots). 650 is the largest multiple of the mix's 50-fixture
  step inside 12,288. 1,000-4,000 are unchanged.

Both shapes are measured at every row; the row's verdict is the worse of the two.

### Gates

Final binary `74505b20…` (default workers, 8 here), three rounds of 8 s after 2 s warmup per row,
paced at the row's rate, medians of the three runs and the worst maximum
(`tl639r8-targets-20261007T020836Z/rows.txt`; load average 1.9-4.9). Pipeline ms.

| Owner target | Workload | p50 | p99 | worst max | Budget | Rate | Misses (3 runs) | RSS |
| --- | --- | ---: | ---: | ---: | ---: | ---: | --- | ---: |
| 8 universes, clearly 60 Hz | typed sustained 1,037 | 4.70 | 5.92 | 9.14 | 16.7 | 60.1 Hz | 0 / 0 / 0 | 91 MB |
| | typed stress 200 | 4.89 | 5.51 | 5.80 | 16.7 | 60.1 Hz | 0 / 0 / 0 | 73 MB |
| 16 universes, clearly 40 Hz | typed sustained 2,074 | 6.84 | 7.39 | 7.77 | 25.0 | 40.1 Hz | 0 / 0 / 0 | 137 MB |
| | typed stress 400 (15 universes) | 7.80 | 8.21 | 8.76 | 25.0 | 40.1 Hz | 0 / 0 / 0 | 121 MB |
| 24 universes, 32-40 Hz (ceiling) | typed sustained 3,111 @ 40 Hz | 8.19 | 9.20 | 18.63 | 25.0 | 40.1 Hz | 0 / 0 / 0 | 188 MB |
| | typed stress 650 @ 40 Hz | 10.14 | 10.71 | 18.89 | 25.0 | 40.1 Hz | 0 / 1 / 0 | 168 MB |
| | typed sustained 3,111 @ 32 Hz | 8.84 | 9.55 | 12.58 | 31.2 | 32.1 Hz | 0 / 0 / 0 | 177 MB |
| | typed stress 650 @ 32 Hz | 10.56 | 11.14 | 11.49 | 31.2 | 32.1 Hz | 0 / 0 / 0 | 165 MB |
| Entry-level, stable 40 Hz at 8 universes | see the estimate below | | | | 25.0 | | | |

The one miss (stress 650 at 40 Hz, round 2) is a single late frame in a run whose worst frame took
18.9 ms of its 25 ms; it coincided with a load spike, and the same row had no miss in the other
two rounds. The legacy (scalar) shapes of the same rows run at p99 1.7-4.9 ms with no misses except
one late frame in one run of the 24-universe sustained show (worst frame 34.8 ms, host).

The typed frame is 3-4× the legacy frame at every size (8 universes: 4.7 / 4.9 ms against 1.7 /
1.5 ms at p50), and a paced frame is slower than an unpaced one: at 40 Hz on one thread the typed
8-universe sustained frame takes 6.96 ms at p50 against 2.81 ms at 240 Hz (stress 200: 7.12 against
4.17 ms). Between frames the cores idle and clock down, and the frame starts cold; the legacy frame
shows the same effect at a smaller scale (1.96 against about 1.5 ms).

### Entry-level estimate (hard requirement: stable 40 Hz at 8 universes)

No entry-level machine was available. The estimate runs the 8-universe rows at 40 Hz with 1, 2 and
4 output workers (an entry-level CPU has 2-4 cores; the default is the available parallelism
capped at 8), once normally and once under `taskpolicy -b` (background QoS: the frame runs on the
slower core cluster at its lowest clocks, so no turbo is left in the measurement), two rounds of
8 s (`entry.txt`). Calibration on the same host: a single-threaded integer loop takes 1.80 / 1.86 s
normally and 7.41 / 7.45 s under `taskpolicy -b` (4.0-4.1×); the unpaced typed 8-universe frames
on one thread take 2.81 → 10.57 ms (sustained, 3.8×) and 4.17 → 19.25 ms (stress, 4.6×). The
assumed entry-level CPU (Intel N100 / Celeron or Ryzen 3 7320U class) is about 3.2-3.5× slower per
thread than this machine's (Apple M5 Max) fastest cores at full clock; background QoS is therefore
a slightly pessimistic stand-in (≈1.2×) for its compute.

Background QoS also throttles the benchmark's sleeps, so the achieved rate (7-17 Hz) and the
misses of those runs are pacing artifacts (the legacy frames, at 2 ms, show the same rates); the
measure is each frame's pipeline time against the 25 ms budget. p99, medians of two runs:

| Workers | Typed sustained 1,037 | Typed stress 200 | Legacy sustained / stress | Headroom (worst typed) |
| ---: | ---: | ---: | ---: | ---: |
| 1, normal | 8.68 | 7.83 | 2.50 / 1.67 | 2.9× |
| 2, normal | 7.61 | 7.62 | 2.03 / 1.64 | 3.3× |
| 4, normal | 6.82 | 8.09 | 1.85 / 1.67 | 3.1× |
| 1, background QoS | 13.54 | 21.43 | 2.92 / 1.86 | 1.17× |
| 2, background QoS | 12.16 | 17.44 | 2.33 / 2.04 | 1.43× |
| 4, background QoS | 11.02 | 13.07 | 2.16 / 1.85 | 1.91× |

Two readings bracket the entry-level frame:

- **Background QoS as the entry-level core** (scaling factor ≈1, slightly pessimistic): the worst
  typed p99 is 13.1 ms with 4 workers and 17.4 ms with 2, so 40 Hz holds with 1.9× and 1.4×
  headroom; on a single thread 21.4 ms leaves 1.17×.
- **This machine's paced frame × 3.5** (the per-thread ratio applied on top of the paced, already
  clocked-down frame, which counts the idle penalty twice if the entry-level CPU also ramps): 8.09 ×
  3.5 = 28.3 ms for the typed stress mix with 4 workers and 6.82 × 3.5 = 23.9 ms for the sustained
  show, so the stress mix would miss its 25 ms by about 13 %.

So the hard requirement is met for the sustained show under both readings and for the stress mix
under the first; the typed stress mix with 2-4 workers lies between 0.88× and 1.9× of its budget
depending on how much of the paced idle penalty an entry-level CPU shares. The legacy frames have
at least 8× headroom under both readings. Confirming the requirement needs a run on a real
entry-level machine (`light-benchmark --headless-stress-fixtures 200 --semantic --rate-hz 40` and
`--profile hard-floor --sustained-show --universes 8 --semantic --rate-hz 40`).

### Equivalence

The engine is unchanged; the final binary was still compared with the start head `892b0ee29`
(`lb-ref-head`) by `--digest-ticks` on the 12 workloads of rounds 4-7, plus the two new 24-universe
typed workloads (`hf24`, `s650`, against the first build that could run them) and lifecycle digests
(fades, FixAT, Freeze, Preload GO) of the typed and legacy tiers: 16/16 clean at the default worker
count, at `LIGHT_OUTPUT_WORKERS` 1 and `max`, and with `LIGHT_VERIFY_PLANS=1` (0 fixture mismatches,
DMX equal on every tick, work counters equal on every tick). Nothing is kept across frames, so no
new invalidation tests were needed.

### Where the time goes at 8 universes

`sample` of the final binary, one worker, unpaced (240 Hz), output thread by section:

- **Typed stress 200 (4.2 ms; 54 % of its fixtures animate five typed lanes).** Groups 34 % (the
  animated Colour fit, `CompiledColorFitting::fit`, 6 % of the frame; the retained family
  composition 12 %; the static Colour rows' proofs), frees 16 % (with one worker the retired
  preparation output, `DynamicFamilyPreparationScratch::clear_output`, is freed inline: 4 % of the
  frame plus its drop glue; with two or more workers it is freed on the pool), preparation 16 %
  (the Position forest bundle 6 %, of which `from_position_forest` 3 %), the deferred completion's
  resolution 8 %, render 7 %, Position fits 3 %, Position program preparation 3 %. Allocation, free,
  copy and zeroing together are about 30 % of the frame's CPU: 48,600 allocations and 12.1 MB per
  frame for 200 fixtures (legacy: 322 and 2.1 MB).
- **Typed sustained 1,037 (2.8 ms on one thread; four workers, sections by wall time).** Static
  Colour rows (the groups section) 26 %, render 13 % (the shared projection; the DMX encoding's
  SipHash map 1 %), the Playback tick 12 % (`ContributionContext::extend_playback`, shared with
  the legacy frame), static targets 7 %, static resolution 6 %, preparation 5 %, accepted Colour
  record 4 %.

Candidates measured and not built, by share of the binding frame (typed stress 200, one thread):
the kept compiled Position graph (round 7, item 1; at most the 6 % forest bundle), reuse of the
retired preparation output's inner buffers instead of freeing them (4 %, only on one thread),
the colour solver's per-solve zeroing of its 16 × 16 matrices (`Qp::new` and `subproblem`, 1.2 %),
the attribute-name lookup per head value in the shared render (`AttributeTable::id`, 2 %), and the
validation of internally built values every frame (`DynamicSampleExpression`, `ColorIntent`,
`AngleNumericProgram` and `DynamicValueAddress` validation, together 6 %, which would need a proof
that the values cannot be invalid). Together they could take the single-thread frame down by
roughly a fifth, which would close the conservative reading's 13 % gap; each needs its own
byte-identity proof.

### Allocation per frame

Counting allocator (`allocs-r8.txt`), objects and bytes per frame, paced at 40 Hz:

| Workload | 1 worker | 4 workers |
| --- | ---: | ---: |
| Typed sustained, 8 universes | 11,491 / 5.7 MB | 12,378 / 8.6 MB |
| Typed stress 200 | 48,596 / 12.1 MB | 51,556 / 17.8 MB |
| Typed sustained, 24 universes | 33,887 / 16.0 MB | 35,183 / 24.9 MB |
| Typed stress 650 | 155,333 / 39.3 MB | 163,167 / 58.0 MB |
| Legacy sustained, 8 universes | 47 / 1.9 MB | 48 / 1.9 MB |
| Legacy stress 200 | 322 / 2.1 MB | 322 / 2.1 MB |

Bounded and proportional to the animated targets, as in round 7. Resident memory at the rows: 73-91
MB at 8 universes, 121-143 MB at 16, 165-198 MB at 24 (legacy 25-63 MB).

### Measurement identities

- Start head and digest reference: `892b0ee29`, binary `d01ec0be…`
  (`.artifacts/tmp/tl639r8/bin/lb-ref-head`); the 24-universe digests' base is `f30269cad`'s
  first build (`lb-cap0`, the same engine).
- Final: `15a0003f5`, binary `74505b20…` (`lb-final`), `--release --locked --no-default-features`.
- Host: Apple M5 Max (6 + 12 cores, 64 GB), shared with the desktop session and an idle debug
  `light-headless` of another worktree; load average 1.9-4.9 during the campaign.
- Evidence: `.artifacts/performance/semantic-output/tl639r8-targets-20261007T020836Z/` (`rows.txt`,
  `entry.txt` with the calibration lines, `host.txt`, `allocs-r8.txt`, `digest-summary.txt`).
  Scripts in `.artifacts/tmp/tl639r8/`: `tier.sh` (one row), `grid.sh`, `campaign.sh`,
  `digest-all.sh` / `dsum.sh` / `wdigest.sh` (with `hf24` and `s650`), `allocs.sh`, `profile.sh`
  and `p6.sh` (`sus8`, `st200`, `sus24`, `st650`), `prof.py` and `dm` (a `rustc-demangle` filter
  for `sample` reports; round 7's helpers in `.artifacts/tmp/tl639/` no longer exist). Profiles
  `p-w1-st200.*`, `p-w4-st200.*`, `p-w4-sus8.*`.

## Start Now latency at the round-8 tiers (TL-641)

The contract: a Dynamic started with Start Now (not on a beat or upbeat boundary) reaches output no
later than the DMX frame after the one being generated, within two frame periods of the gesture.
Scope is the round-8 typed tiers above; the 4,000-fixture stress target and show load are out of
scope.

### How it was measured

`light-benchmark --start-latency --semantic` on both round-8 shapes (the sustained show now takes
the probe as well as the headless stress mix), release `--locked --no-default-features`, three runs
per row, medians. The probe starts one Start Now Dynamic on every target the way the desk does: the
runtime controller under the Dynamics lock, then the `DynamicOn` rows under the Programmer lock.

- **Serialized:** the gesture between two frames, then the first frame split into reconcile and
  merge, controller construction, and family evaluation plus render.
- **Concurrent:** an output thread paced at the row's rate and the gesture issued while a frame is
  in progress. It reports how many frames after the in-progress one first carried the Dynamic, the
  gesture-to-end-of-that-frame time against two frame periods, and the in-progress frame's time
  against the median steady frame (a gesture that blocked the frame would show here).

The frame that carries the start is also read from the desk's change lead ledger (TL-659), the
same claim the DMX statistics measure from; both agree on every run. The sustained show's own
keyframe wave runs from Current to a preset the Live lane has no source for and never moves DMX,
so its probe Dynamic is that wave's Max/Min sine (10-90 %), which changes every fixture from the
first frame.

### Results

Host load average 6-8 from other sessions during the runs (round 8 measured at 1.9-4.9), so frames
run 1.4-1.6× round 8's. Pipeline ms, medians of three runs (`rows-branch.md`):

| Row | Targets | Rate / budget | Programmer apply | Reconcile + merge | Controller construction | Family eval + render | Concurrent gesture → output | Frames after in-progress | Gesture blocks in-progress frame |
| --- | ---: | --- | ---: | ---: | ---: | ---: | ---: | --- | --- |
| 8 u, stress 200 | 388 | 60 Hz / 33.3 | 0.74 | 1.23 | 0.32 | 7.25 | 28.2 | 1 / 1 / 1 | no (2.13 vs 2.22) |
| 8 u, sustained 1,037 | 1,037 | 60 Hz / 33.3 | 0.32 | 0.40 | 0.11 | 4.36 | 4.2 | 0 / 0 / 0 | no |
| 16 u, stress 400 | 776 | 40 Hz / 50.0 | 1.23 | 1.95 | 0.49 | 12.24 | 42.7 | 1 / 1 / 1 | no (3.01 vs 3.56) |
| 16 u, sustained 2,074 | 2,074 | 40 Hz / 50.0 | 0.56 | 0.61 | 0.26 | 8.59 | 9.3 | 0 / 0 / 0 | no |
| 24 u, stress 650 | 1,261 | 40 Hz / 50.0 | 2.63 | 3.72 | 1.11 | 20.70 | 56.2 | 1 / 1 / 1 | no (4.15 vs 4.09) |
| 24 u, sustained 3,111 | 3,111 | 40 Hz / 50.0 | 0.77 | 0.99 | 0.32 | 10.91 | 12.5 | 0 / 0 / 0 | no |

"0" means the in-progress frame itself carried the Dynamic: the gesture reached the Dynamics
runtime before that frame's Dynamics transaction. The Programmer apply runs on the gesture's own
thread, never inside an output frame.

Every row is carried by the in-progress frame or the next one, on every run. The two-period budget
holds on every row except stress 650 at 40 Hz in this loaded series (56.2 ms); re-measured with the
base build interleaved (`rows-st650-*-rerun.md`, load 7.6-8.9) it holds at 41.9 ms (base 46.7 ms,
one of three base runs over). Its carrying frame (19-27 ms) is the steady animated frame (15-17 ms
here, about 10 ms in round 8) plus the start's reconcile and controller construction (about 4 ms),
so the remaining headroom is about 8 ms at round 8's load and less on a loaded host. The base build
(`c8a894b2c`, stress rows only) measures the same within noise (`rows-base.md`), so TL-659's change
lead claims add nothing measurable.

No engine change was made: the contract is met on every row, the miss on the loaded run comes from
the steady animated frame approaching the period, not from the start path, and every candidate
(moving reconciliation out of the frame) would need its own byte-identity proof. Beat and upbeat
boundary starts are untouched; `light-dynamics` tests cover that a boundary start is due at the
boundary sampling chooses, not on the next frame.

### Equivalence

`--digest-ticks 240`, with and without `--digest-lifecycle`, on typed stress 200 / 400 / 650, typed
sustained 8 / 16 / 24 universes and the legacy stress 200 and sustained 8 shapes: 16/16 byte
identical between the base build and the final build (`digests/digest-summary.txt`).

### Measurement identities

- Base `c8a894b2c`, binary `8a75fb31…`; measured: `4a0264bd9` plus the TL-641 probe changes,
  binary `92b090be…` (`binaries.txt`). The commit after it only drops an unused probe parameter
  and adds tests.
- Host: Apple M5 Max, shared with other agents' builds and tests.
- Evidence: `.artifacts/performance/tl641-start-latency/` (per-run JSON under `branch/`, `base/`,
  `branch-quiet/`, `base-quiet/`; `rows-*.md`; `digests/`). Scripts in `.artifacts/tmp/tl641/`
  (`summarize.py`, `digests.py`).
