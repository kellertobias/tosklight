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
