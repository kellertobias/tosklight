# 08c — Cue Fade and Speed Groups

## Outcome

Complete parent step 08's Cue Fade and Speed Group helper families, including reproducible
humanized tap tempo.

## Public helpers

- `timing.cueFade.set/double/half/off(...)`;
- `speedGroup[SpeedGroup.A..E].setBpm/addBpm/subtractBpm/synchronizeFrom(...)`;
- `speedGroup[...].via.click.tapTempo(...)`;
- `speedGroup[...].via.shiftClick.openSettings()` and `.via.hold.openSettings()`;
- normalized Cue Fade, Speed Group source, BPM, synchronization, and phase assertions.

Tap intervals are wall paced, bounded, seeded from the scenario route seed, reported, and
replayable. Direct entry and the first learning tap break synchronization and take manual
ownership.

## Helper-contract scenarios

1. Set, double, halve, and turn off Cue Fade, then prove fallback timing without overriding a
   Cue's explicit Fade.
2. Set and relatively adjust every `SpeedGroup` enum member.
3. Synchronize two groups, then break synchronization with direct entry and tap tempo.
4. Tap a target BPM with seeded bounded human jitter, report the generated intervals, and
   reproduce them from the run seed.
5. Prove ordinary click learns tempo while Shift-click and hold open settings.

## Done gate

- Cue Fade remains distinct from Programmer Fade and per-Cue timing.
- Every Speed Group uses the enum-backed typed surface.
- Tap-tempo behavior is truthful, deterministic, and diagnosable.

## Result

- Added a Cue Fade helper family with set, double, half, off, and normalized authority
  assertions, kept separate from Programmer Fade.
- Added enum-backed helpers for all five Speed Groups, including direct and relative BPM,
  synchronization, source/BPM assertions, deterministic tap reports, Shift-click settings, and
  real hold settings gestures.
- Added focused scenarios proving Cue Fade fallback versus explicit Cue timing, all five Speed
  Group identities, synchronization break rules, replayable humanized tap tempo, and settings
  gesture boundaries.
- Tap intervals advance wall time and the same virtual application-time interval because the
  server intentionally stamps accepted taps with application time.

Verification:

- `npm run test:e2e -- tests/testBench/08c-cue-fade-and-speed-groups.spec.ts` — 2 passed.
- `npm run test:e2e` — 325 passed, 9 skipped.
- `npm --prefix apps/control-ui run typecheck` — passed.
- `npm run test:bench-types` — passed.
- `npm run test:architecture` — passed.
- `node tools/check-source-size.mjs` — passed.
- `git diff --check` — passed.
