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
