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
