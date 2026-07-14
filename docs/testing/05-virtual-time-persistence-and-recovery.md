# Virtual Time, Persistence, and Recovery

These scenarios focus on behavior that is slow or flaky under wall time and on failures that must not damage operator data.

## How to run this file

Timing cases run in `--test-bench`; persistence cases use a dedicated serial fixture and real temporary files. Every case loads a canonical show, immediately uses Save As to create its named working copy, and applies mutations only to that copy. Before restart, record file hashes, active show ID, revision, programmer/playback state, and expected output. After restart, wait independently for readiness and bootstrap. Desktop cases launch the packaged app as a child process with unique data and port environment variables and assert process ownership explicitly.

## TIME-001 — Zero tick emits without advancing

**Priority:** P0  
**Primary layer:** Server E2E

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `time-001.show`, and use the active copy for this scenario.

**Actions:** Reset the clock, program an immediate value, mark all receivers, and advance by 0 ms twice.

**Assertions:** Both calls report `2020-01-01T00:00:00Z`. Each call emits exactly one current frame per enabled route and one OSC feedback cycle. Protocol sequences increment, while all behavior timestamps remain unchanged.

**Pass condition:** Tests can observe current state repeatedly without introducing application-time movement.

## TIME-002 — Fade boundaries are exact

**Priority:** P0  
**Primary layer:** Rust integration plus selected E2E

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `time-002.show`, and use dimmer fixture 1 in the active copy.

**Setup:** Fade intensity from 0 to 100% over 3,000 ms.

**Checkpoints:** 0, 1, 1,499, 1,500, 1,501, 2,999, 3,000, and 3,001 ms.

**Assertions:** Values are monotonic, midpoint rounding is documented, the endpoint is exactly 255, and no wall-time delay changes a checkpoint.

**Pass condition:** Interpolation is reproducible at boundaries and finishes exactly once.

## TIME-003 — Chaser and effect phases survive large jumps

**Priority:** P1  
**Primary layer:** Rust integration

**Starting show:** Load canonical `default-stage.show`, immediately Save As `time-003.show`, and use the active copy. Create a new cue list on fixtures 501.1–501.10 for the chaser and use fixture 501's color emitters for the periodic effect.

**Cases:** Advance one step at a time, jump across several steps, change speed mid-step, pause/resume, and jump seven days. Repeat for a periodic effect phase.

**Assertions:** The result is defined by virtual timestamp and configured phase rules, not by the number or duration of scheduler iterations. Large jumps do not create an unbounded catch-up loop.

**Pass condition:** A direct jump and an equivalent series of smaller advances end in the same defined state.

## SHOW-001 — Save, restart, and reopen

**Priority:** P0  
**Primary layer:** Serial server E2E

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `show-001.show`, and use the active copy for this scenario.

**Setup:** Edit the loaded groups, record cues, save, leave a playback active, and place an unrecorded value in the durable user's programmer.

**Actions:** Stop the server cleanly, start a new process against the same temporary data directory, reconnect, and open the show.

**Assertions:** Patch, routes, ordered groups, presets, cues, active show identity, and documented durable programmer/playback state reload correctly. No files appear in repository `light-data`.

**Pass condition:** A normal restart preserves durable state without converting transient values into show data.

## SHOW-002 — Crash during save recovers atomically

**Priority:** P1  
**Primary layer:** Storage integration

**Starting show:** For each injected-failure case, load canonical `compact-rig.show`, immediately Save As `show-002-<case>.show`, and use only that active copy. Save one valid baseline edit before injecting the failure on the next save.

**Actions:** Inject failure before replacement, during temporary-file write, and after replacement but before backup cleanup.

**Assertions:** On restart the server opens either the complete old revision or complete new revision, never truncated or mixed JSON. Recovery and backup choice are logged and exposed to the operator.

**Pass condition:** Interrupted persistence cannot destroy the last known-good show.

## SHOW-003 — Invalid active show enters recovery

**Priority:** P0  
**Primary layer:** Server integration

**Starting show:** For each invalid-show case, load canonical `compact-rig.show`, immediately Save As `show-003-<case>.show`, retain an untouched backup of that working copy, and then make the active copy malformed, schema-invalid, or referentially invalid.

**Setup:** Persist the mutated active copy while retaining the valid compact-show backup or a separately imported fresh copy.

**Assertions:** Server readiness succeeds, output remains safe, bootstrap reports the active-show error, and the operator can select a valid show. The invalid file is not overwritten merely by startup.

**Pass condition:** One bad show cannot prevent the desk from starting or silently erase recovery evidence.

## SHOW-004 — Backward-compatible migration is stable

**Priority:** P1  
**Primary layer:** Fixture-based Rust integration

**Starting show:** For each migration case, load canonical `compact-rig.show`, immediately Save As `show-004-<case>.show`, rewrite only the named fields in the active copy into the representative historical shape, and save that derived legacy fixture before testing migration.

**Cases:** Load representative old shows missing newer fixture numbers, group fields, playback fields, route fields, and virtual-dimmer metadata. Save and reopen the migrated result.

**Assertions:** Defaults are documented, identities and addressing remain stable, migration is idempotent, and the second open performs no additional semantic rewrite.

**Pass condition:** Supported historical shows migrate once without losing operator intent.

## DESKTOP-001 — Packaged app owns its child server

**Priority:** P0  
**Primary layer:** macOS packaged smoke

**Starting show:** Load canonical `default-stage.show`, immediately Save As `desktop-001.show` in the packaged app's temporary data directory, and make that working copy the active show before launch.

**Actions:** Launch the bundled Tauri app with a temporary data directory and unique loopback port.

**Assertions:** The app-owned server reaches readiness; the WebView connects, bootstraps a session, and renders the desk; the frontend-ready marker is written; quitting the app terminates its exact child server; the loopback port closes within the process deadline.

**Pass condition:** Packaging, frontend bootstrap, server ownership, and shutdown work without touching normal operator data or an already-running production server.

## DESKTOP-002 — Existing server is not adopted as a child

**Priority:** P2  
**Primary layer:** macOS process integration

**Starting show:** Load canonical `default-stage.show`, immediately Save As `desktop-002.show`, and start the independent server with that working copy as its active show.

**Setup:** Start a server independently, then launch the desktop app configured to use it.

**Actions:** Verify readiness, quit the app, and query the independent server again.

**Assertions:** Record app and server PIDs before quit. The app PID exits, the independent PID remains unchanged, readiness still succeeds, and its data directory and log remain writable by that server.

**Pass condition:** The desktop app terminates only a server process it spawned and never kills an independently owned server.

## Follow-ups

| Scenario | Next tests after the primary case | First failure checks |
| --- | --- | --- |
| TIME-001 | Tick with no routes, multiple routes, and multiple OSC subscribers. | Compare returned frame count, receiver marks, OSC burst count, and unchanged clock. |
| TIME-002 | Repeat for delay, LTP, group master fade, and cue crossfade. | Record exact virtual timestamp and unrounded engine value at the first mismatch. |
| TIME-003 | Test negative/oversized advance rejection and overflow-safe far-future dates. | Separate deadline calculation from phase evaluation and loop count. |
| SHOW-001 | Repeat with unclean termination and with no active show. | Compare pre/post file hashes, store records, bootstrap, and first output frame. |
| SHOW-002 | Test backup retention and disk-full/permission failures. | Preserve directory listing, hashes, and recovery log before cleanup. |
| SHOW-003 | Add missing assets and partially migrated references. | Identify parse, validation, compile, or activation stage from diagnostics. |
| SHOW-004 | Maintain one fixture per supported historical schema version. | Diff semantic normalized JSON before and after each migration pass. |
| DESKTOP-001 | Repeat release packaging and normal user quit paths. | Record app PID, child PID, marker, readiness timeline, and port closure. |
| DESKTOP-002 | Test an independently started server crash while the app remains open. | Verify ownership flag before any kill/restart behavior. |
