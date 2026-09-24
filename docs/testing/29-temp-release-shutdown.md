# Temp Release Shutdown

## Purpose

Prove that a Temp plays its Cuelist with the Cues' own timing and follows on by itself. Once the
list's last Cue has released everything it held, the playback turns itself off, and so does its
Temp button.

Fixtures used:

- **Sunstrip 1**: `Showtec › Sunstrip Active DMX`, mode `10 Channel`, ten dimmer lamps. It gives
  the strip's level on DMX that rests dark.
- **Sunstrip RGB 2**: `Showtec › Sunstrip LED RGB 42206`, mode `30 Channel`. It gives the strip's
  colour.

## BENCH-TEMP-RELEASE-001 — Temp, follow, release, off

Automated in `tests/115-semantic-temp-release-shutdown.spec.ts`. The same timing is checked at
engine level, on authoritative resolved values, in
`crates/light/domain/engine/tests/temp_release_shutdown.rs`.

1. Record Cue 1 with both strips full up and the RGB strip white.
2. Record Cue 2 with every attribute of both strips released (`FIXTURE 1 THRU 2 [^OFF] [^0]`).
3. Give Cue 1 an In Fade of 0.2 s and an Out Fade of 1 s.
4. Give Cue 2 a Follow trigger, 0.3 s after Cue 1 completes, and an In Fade of 1 s.
5. Configure the Playback with a single **Temp** button. Assign it to a Virtual Playback cell.
6. Press Temp once:
   - after 0.1 s the lamps are about half up;
   - after 0.2 s they are full.
7. After the 0.3 s hold, Cue 2 follows without another press, and the lamps fade out.
8. After the 1 s release, the lamps are dark and the Playback is off.
9. Press Temp again. It starts the list afresh rather than switching Temp off, which proves the
   button had returned to off.
