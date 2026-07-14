# Cues, Tracking, and Arbitration

Use `default-stage.show` for these cue, color, position, and arbitration cases.

## How to run this file

Before every scenario, load canonical `default-stage.show`, immediately use Save As with the filename stated by that scenario, and use only the active working copy. Where a scenario uses group numbers, create group 1 `Front Fresnels` from fixtures 1–6, group 2 `Back Profiles` from fixtures 101–108, and group 3 `Back LED Washes` from fixtures 201–205 as part of that scenario's setup. Build the cue list through the named surface, then clear the programmer and release playback before verification. Capture expected tracked state from cue data, not current output. For each GO, jump, pause, or release, wait for the playback revision, advance to exact virtual checkpoints, and compare playback state, resolved attributes, and received DMX.

## CUE-001 — Record and replay a tracked cue sequence

**Priority:** P0  
**Primary layer:** Playwright E2E

**Starting show:** Load canonical `default-stage.show`, immediately Save As `cue-001.show`, and use the active copy for this scenario.

**Setup:** Create groups 1–3 as described above. No programmer or playback is active. Create an empty sequence on playback 1.

**Actions:**

1. Record cue 1 with Front Fresnels at 40%, Back Profiles at 30%, and a warm color on the Back Profiles.
2. Record cue 2 changing only Back Profiles to 70%.
3. Record cue 3 changing only the color to blue.
4. Clear the programmer, release the playback, and run cues 1–3.

**Assertions:** Cue 2 retains the cue 1 Front Fresnel value and warm Back Profile color. Cue 3 retains the cue 2 intensities while changing only color. Current/next UI, playback API, resolved attributes, and UDP output agree.

**Pass condition:** Omitted values track forward and playback reconstruction does not depend on residual programmer state.

## CUE-002 — Cue-only restores the previous state

**Priority:** P0  
**Primary layer:** Rust integration plus selected E2E

**Starting show:** Load canonical `default-stage.show`, immediately Save As `cue-002.show`, and use the active copy for this scenario.

**Setup:** Create group 1 `Front Fresnels` from fixtures 1–6. Cue 1 sets group 1 to 30%. Cue 2 is recorded cue-only at 80%. Cue 3 changes an unrelated attribute or fixture.

**Actions:** Run cues sequentially and then jump directly to each cue from release.

**Assertions:** Cue 2 outputs 80%. Cue 3 restores group 1 to 30%. Direct jumps reconstruct the same state as sequential GO operations.

**Pass condition:** Cue-only restoration is deterministic and independent of navigation history.

## CUE-003 — GO, back, pause, resume, and release

**Priority:** P0  
**Primary layer:** Playwright E2E

**Starting show:** Load canonical `default-stage.show`, immediately Save As `cue-003.show`, and use the active copy for this scenario.

**Setup:** Create group 1 `Front Fresnels` from fixtures 1–6. Cue 1 puts group 1 at 0%; cue 2 puts it at 100% with a 4,000 ms fade.

**Actions and virtual checkpoints:**

- GO cue 2; assert 0 at 0 ms, approximately 128 at 2,000 ms, and 255 at 4,000 ms.
- Start again, pause at 1,000 ms, advance the clock by 10,000 ms, and assert the value is unchanged.
- Resume and advance the remaining 3,000 ms.
- GO minus and verify the previous tracked state.
- Release and verify restoration to the next authoritative source.

**Assertions:** Current cue, next cue, paused state, transition timestamps, and DMX values match every checkpoint. During the 10,000 ms paused jump, both engine values and packet bytes remain at the paused level.

**Pass condition:** Navigation and pause state use application time, with no progress caused by wall time or a large paused-time jump.

## CUE-004 — Delays and split fade boundaries

**Priority:** P1  
**Primary layer:** Rust integration

**Starting show:** Load canonical `default-stage.show`, immediately Save As `cue-004.show`, and use the active copy for this scenario.

**Setup:** Create group 2 `Back Profiles` from fixtures 101–108. Record a cue for that group with a 1,000 ms delay, 3,000 ms intensity fade, and 2,000 ms color/position LTP fade.

**Checkpoints:** Assert immediately before, exactly at, and immediately after the delay and both fade endpoints.

**Assertions:** Before the delay ends, output equals the source state. At each fade start and endpoint, intensity and LTP attributes equal their independently calculated values without a one-frame early or late transition.

**Pass condition:** Delay and fade boundary behavior is explicit, stable at millisecond resolution, and consistent for direct jump and sequential playback.

## CUE-005 — Follow and wait do not depend on wall time

**Priority:** P1  
**Primary layer:** Rust integration plus selected E2E

**Starting show:** Load canonical `default-stage.show`, immediately Save As `cue-005.show`, and use the active copy for this scenario.

**Setup:** Create group 1 `Front Fresnels` from fixtures 1–6. Record cue 1 to follow to cue 2 after 5,000 ms; cue 2 waits for an explicit GO before cue 3.

**Actions:** Advance to 4,999 ms, 5,000 ms, and then by a seven-day jump.

**Assertions:** Cue 2 activates exactly at 5,000 ms. The large jump does not bypass the explicit wait or repeatedly trigger transitions.

**Pass condition:** Automatic transitions obey exact virtual deadlines and process large jumps predictably.

## MERGE-001 — Two programmers compete by priority and recency

**Priority:** P0  
**Primary layer:** Rust/API integration

**Starting show:** Load canonical `default-stage.show`, immediately Save As `merge-001.show`, and use the active copy for this scenario.

**Cases:**

- Equal-priority intensity uses HTP.
- Higher-priority intensity wins where priority is defined to override HTP scope.
- Equal-priority LTP uses the most recent operator edit timestamp.
- Re-rendering without an edit does not change LTP ownership.
- Releasing the winning programmer reveals the next contribution.

**Assertions:** For every case, compare the complete ordered contribution set, chosen source per attribute, normalized resolved value, and application edit timestamp. Re-rendering without mutation must produce an identical result.

**Pass condition:** Priority, HTP, and LTP decisions use operator edit time and never render-loop timing.

## MERGE-002 — Programmer and two playbacks arbitrate correctly

**Priority:** P1  
**Primary layer:** Rust integration plus selected E2E

**Starting show:** Load canonical `default-stage.show`, immediately Save As `merge-002.show`, and use the active copy for this scenario.

**Setup:** Use Back Profiles 101–108. Two playbacks and one programmer contribute intensity, color, and position to overlapping fixtures.

**Actions:** Activate sources in different orders, change playback masters, release each source, and repeat with direct cue jumps.

**Assertions:** Intensity follows HTP rules. Color and position follow LTP priority/recency rules. Releasing a source restores the correct underlying value without a transient zero frame.

**Pass condition:** Arbitration is independent of activation path and stable across programmer/playback releases.

## Follow-ups

| Scenario | Next tests after the primary case | First failure checks |
| --- | --- | --- |
| CUE-001 | Add preset references, group membership edits, and save/restart. | Compare stored cue deltas, reconstructed tracked state, and active contribution order. |
| CUE-002 | Put cue-only changes at the first/last cue and combine them with tracked group references. | Inspect the generated restoration delta before testing navigation. |
| CUE-003 | Repeat with GO from OSC and REST, and with a non-zero cue delay. | Compare virtual timestamps and playback transition state at the first wrong checkpoint. |
| CUE-004 | Add split up/down intensity times and per-attribute delays. | Assert delay start/end independently from interpolation start/end. |
| CUE-005 | Add follow loops, disabled follow, and speed changes before the deadline. | Inspect scheduled application-time deadline and transition count. |
| MERGE-001 | Permute source creation/edit order and equal timestamps. | Dump normalized contributions with priority and edit timestamp before resolution. |
| MERGE-002 | Add group masters, blackout, and grand master. | Separate source arbitration from final intensity scaling and DMX encoding. |
