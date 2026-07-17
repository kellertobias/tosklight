# Move in Black Implementation Contract

This document defines the implementation required by [MIB-001 in the Cue and playback tests](../../../tests/07-move-in-black.spec.ts). Implement the patch schema, playback look-ahead, and UI controls here before enabling the matching Playwright test.

## Operator behavior

Move in Black prepositions a moving fixture for its next lit Cue while the fixture is dark.

For the basic sequence:

- Cue 1: fixture is on at Position A.
- Cue 2: fixture fades to intensity zero.
- Cue 3: fixture is on at Position B.

the fixture stays at Position A while Cue 2's intensity is fading. Once its actual resolved dimmer reaches zero, its configured Move in Black delay begins. After that delay, the fixture moves toward Position B while Cue 2 remains current. When Cue 3 is triggered, the fixture is already at Position B whenever the dark interval was long enough.

Move in Black applies only to Position-family attributes such as pan and tilt. It does not pre-run Color, Beam, Intensity, or arbitrary attributes.

## Patch configuration

Every patched fixture has:

- **Move in Black**: enabled by default.
- **Move in Black Delay**: a non-negative duration in seconds in the UI and milliseconds in persisted data. Default `0`.

Add **MIB** and **MIB Delay** cells to each fixture row in **Setup → Patch**. They use the same selected-cell editing model as the rest of the patch: an ordinary click only selects the fixture, while `[SET]` followed by the cell opens the editor. They remain fixture-level settings even when the fixture has logical heads or multipatches; all heads and physical instances inherit the parent fixture's behavior. Fixtures without Position attributes still expose the setting for consistency, but it has no runtime effect.

Edits use the existing revision-checked patch update path, survive Save/Reload, and update the live engine without repatching or restarting the show.

## Suggested patch schema

The exact field names may follow repository conventions, but `PatchedFixture` must persist equivalent information:

```text
move_in_black_enabled: boolean = true
move_in_black_delay_millis: non-negative integer = 0
```

Missing fields in old shows default to enabled with zero delay. Export/import, Save As, named revisions, MVR round trips where show-local extension data is retained, and patch duplication must preserve the settings.

## Finding the target position

When a Cuelist enters or approaches a dark interval, reconstruct its tracked future state in numeric Cue order:

1. Begin after the current Cue.
2. Find the next Cue in which the fixture's reconstructed Intensity becomes greater than zero.
3. Resolve the fixture's complete tracked Position state at that Cue.
4. If that future lit state differs from the current Position, use it as the Move in Black target.
5. If there is no later lit Cue, no Position state, or no Position change, do not create a Move in Black transition.

The target comes from the reconstructed future Cue state, not merely from Position values explicitly stored in that one Cue. A chain of several dark Cues therefore prepositions toward the next eventual lit state. Do not look across Wrap Around Off. Tracking/Reset wrap look-ahead requires its own boundary tests before it is enabled across the end of a Cuelist.

## When darkness begins

Do not start Move in Black merely when an Off Cue is triggered. Start its delay only at the application timestamp where the fixture's resolved Intensity reaches exactly zero after its intensity fade and all active programmer/playback arbitration.

If another programmer or playback still keeps the fixture above zero, Move in Black remains blocked. When that last light-producing contribution is released and resolved Intensity reaches zero, start the delay from that later timestamp.

If Intensity rises above zero again during the delay or hidden move, cancel the Move in Black contribution immediately. Normal live Cue/programmer Position arbitration then applies. Returning to zero starts a fresh complete delay; it does not resume a partially elapsed delay.

## Hidden movement timing

There is no separate Move in Black fade setting. After the Move in Black delay:

- use the next lit Cue's explicit per-value Position fade when present;
- otherwise use that Cue's master Fade;
- otherwise use the configured Cue/sequence fade fallback.

The hidden transition begins after the fixture is dark plus the Move in Black delay. It is not postponed until the next Cue is triggered. At the next Cue, the normal Position contribution takes ownership at the same target without jumping backward or restarting an already completed move.

Disable Cue Timing on the driving Cuelist treats the hidden Position fade as zero but does not bypass the fixture's separate Move in Black delay. The patch delay is an MIB safety delay, not Cue timing.

## Arbitration and source ownership

Move in Black is an internal Position contribution owned by the driving Cuelist and fixture. It exists only while that fixture is dark and that Cuelist has a valid future target. It must:

- respect numeric priority and normal LTP Position arbitration;
- never claim Intensity;
- yield to a higher-priority or newer legitimate Position source according to normal arbitration;
- disappear when cancelled, when the target Cue becomes active, when the Cuelist is released, or when the target is invalidated by a Cue edit;
- reveal the correct underlying Position without an intermediate default frame.

If several active Cuelists propose different future Move in Black targets, normal priority/LTP rules choose the winning Position contribution. The losing proposals remain non-authoritative and must not mutate Cue data.

## Runtime recalculation

Recalculate or invalidate the pending target when:

- a future Cue is inserted, deleted, moved, copied, or edited;
- tracking or Cue-only changes alter the next lit Position state;
- the Cuelist advances, wraps, restarts, or is released;
- the fixture's MIB enable/delay setting changes; or
- another source changes whether the fixture is actually dark.

Never persist a calculated MIB transition into Cue data. It is runtime state derived from patch configuration and the current tracked Cuelist.

## Existing implementation seams

- `crates/fixture/src/lib.rs` owns `PatchedFixture` and needs backward-compatible MIB fields and validation.
- `apps/control-ui/src/api/types.ts` mirrors the patched-fixture schema.
- `apps/control-ui/src/components/setup/FixturePatchSetup.tsx` owns the Patch table and revision-checked fixture edits; add the per-row controls there.
- `crates/playback/src/lib.rs` owns tracked Cue reconstruction, active Cuelists, position arbitration, and transition timing. MIB look-ahead and runtime state belong there or in a focused playback module, not in React.
- `crates/engine/src/lib.rs` owns resolved output and transitions. It must provide the exact resolved-dark boundary and render the hidden Position transition without DMX discontinuities.
- `crates/server/src/main.rs` must propagate patch-setting and Cue edits into the active engine and expose MIB runtime evidence needed by the test harness.

## Required runtime evidence

Expose enough normalized state for tests and diagnostics to identify:

- fixture ID;
- driving Cuelist and current Cue;
- future target Cue;
- current and target Position;
- dark-since timestamp;
- delay deadline;
- movement start/end timestamps;
- active, blocked, cancelled, or completed state.

The test must not infer MIB solely from final pan/tilt DMX because that cannot distinguish correct look-ahead from an accidental early Cue execution.

## Implementation order

1. Add patch schema, defaults, validation, and old-show migration.
2. Add Patch UI controls and revision-checked persistence.
3. Add tracked future-position lookup and invalidation.
4. Add resolved-dark detection, delay scheduling, transition ownership, and cancellation.
5. Add Rust tests for timing, arbitration, several-dark-Cue look-ahead, and migration.
6. Expose normalized runtime evidence through the server test seam.
7. Enable MIB-001 UI/API coverage only after the feature exists end to end.
