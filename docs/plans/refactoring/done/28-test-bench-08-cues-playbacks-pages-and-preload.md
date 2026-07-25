# 08 — Cues, Playbacks, pages, and Preload

## Outcome

Add recording and runtime helpers for Cuelists, Cues, Playbacks, pages, and Preload. Preserve
current-page versus explicit-page addressing and the press/release nature of temporary controls.

## Public helpers

- `record.playback(number, options?)`;
- `record.cue({ playback, cue, mode, timing })`;
- `cue.update/delete/move/copy/goto/load/select(...)`;
- `playback.go/goBack/on/off/toggle/release(...)`;
- `playback.flash/temp/swap(...)`;
- `playback.fader(number, value)`;
- `playback.select(number)`;
- `playback.configure(number, definition)`;
- `playback.map({ page, slot, playback })`;
- `page.select/next/previous/create/rename(...)`;
- `preload.start/commit/clear(...)`;
- normalized Cue, Playback, active-Cue, page, and Preload assertions.

Use enums for recording modes, playback functions, fader/button behavior, page ownership, and
timing fields where a closed product vocabulary exists.

## Desk timing and Speed Groups

The global Cue Fade fallback is separate from a Cue's own master Fade and from Programmer Fade:

```ts
await t.timing.cueFade.set("3s");
await t.timing.cueFade.double();
await t.timing.cueFade.half();
await t.timing.cueFade.off();
```

The visible route operates the real Cue Fade control and waits for authoritative feedback.
Scenarios use the Cue editing helpers for a Cue-specific Fade or Delay.

Speed Groups use an enum and expose direct, relative, synchronized, and tap-tempo operations:

```ts
export enum SpeedGroup {
  A = "A",
  B = "B",
  C = "C",
  D = "D",
  E = "E",
}

await t.speedGroup[SpeedGroup.A].setBpm(120);
await t.speedGroup[SpeedGroup.B].addBpm(5);
await t.speedGroup[SpeedGroup.C].subtractBpm(2.5);
await t.speedGroup[SpeedGroup.D].synchronizeFrom(SpeedGroup.A);

await t.speedGroup[SpeedGroup.E].via.click.tapTempo({
  bpm: 128,
  taps: 6,
  jitter: {
    maximum: "30ms",
    distribution: TapJitter.Uniform,
  },
});
```

`tapTempo` is wall paced and performs real taps around the ideal `60_000 / bpm` interval.
Jitter is bounded, seeded from the scenario route seed, reproducible, and reported with the actual
generated intervals. The generator keeps the average close enough to the target BPM that the
assertion can use a declared tolerance while still resembling a human operator. It must not use
unseeded randomness or make CI timing failures unreplayable.

Ordinary click/tap learns tempo. The settings action exposes the distinct modifier/hold routes:

```ts
await t.speedGroup[SpeedGroup.A].via.shiftClick.openSettings();
await t.speedGroup[SpeedGroup.A].via.hold.openSettings();
```

Direct BPM entry or the first learning tap must break Speed Group synchronization and take manual
ownership as documented.

## Addressing types

The type system distinguishes:

- current-page slot;
- explicit page plus slot;
- concrete Playback number;
- named independently paged screen plus slot.

A page change must not silently change a concrete Playback target. A current-page action must use
the current page at the moment of execution.

## Gesture contracts

Flash, Temp, Swap, encoder presses, and other momentary actions preserve down/up phases. The
helper exposes a readable hold form when duration matters and uses the real UI or OSC press and
release. It does not collapse a gesture to a final state mutation.

## Helper-contract scenarios

1. Record two Cues, run them, and assert logical DMX at exact clock boundaries.
2. Update, move, copy, and delete a Cue through visible and typed routes.
3. Address the same slot before and after a page change and prove current-page semantics.
4. Address a concrete Playback across page changes.
5. Configure and map Playback buttons/faders through typed definitions.
6. Exercise GO, GO BACK, ON, OFF, TOGGLE, and RELEASE.
7. Prove Flash, Temp, and Swap down/up behavior through UI and OSC.
8. Move a fader and assert HTP/runtime behavior without generalizing it to Programmer LTP.
9. Start, inspect, commit, and clear Preload.
10. Change an independently paged browser screen and prove Main remains unchanged.
11. Set, double, halve, and turn off Cue Fade, then prove fallback timing without overriding a
    Cue's explicit Fade.
12. Set and relatively adjust every `SpeedGroup` enum member.
13. Synchronize two Speed Groups, then break synchronization with direct entry and tap tempo.
14. Tap a target BPM with seeded bounded human jitter, report the intervals, and reproduce them
    from the run seed.
15. Prove ordinary click learns tempo while Shift-click and hold open settings.

## Done gate

- Every runtime action has an unambiguous typed target.
- Temporary controls retain gesture phases.
- Page addressing, Programmer LTP, and playback/cue HTP behavior are not conflated.
- Recording scenarios use helpers from earlier steps and contain no direct object writes.

## Result

Split at execution time into three independently gated chunks:

- `08a-cue-playback-recording-and-runtime.md` owns recording, Cue editing, concrete Playback
  runtime actions, configuration, and normalized assertions.
- `08b-pages-preload-and-momentary-playbacks.md` owns current/explicit page addressing,
  mapping, independent screen pages, Preload, and Flash/Temp/Swap gesture phases.
- `08c-cue-fade-and-speed-groups.md` owns Cue Fade, all five Speed Groups, synchronization,
  reproducible tap tempo, and settings gestures.

The split follows existing production authority boundaries and preserves every public helper,
scenario, and done-gate requirement from this parent plan.
