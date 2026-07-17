# Virtual Time, Persistence, and Recovery

These scenarios focus on behavior that is slow or flaky under wall time and on failures that must not damage operator data.

**Automated coverage:** Implemented by [`tests/05-virtual-time-persistence-and-recovery.spec.ts`](../../tests/05-virtual-time-persistence-and-recovery.spec.ts) and the Playwright process-integration cases in [`tests/05-desktop-process-integration.spec.ts`](../../tests/05-desktop-process-integration.spec.ts). Run the packaged cases through `./test desktop-smoke`; that command builds and launches the actual macOS application bundle rather than substituting a browser page.

## How to run this file

Timing cases run in `--test-bench`; persistence cases use a dedicated serial fixture and real temporary files. Every case loads a canonical show, immediately uses Save As to create its named working copy, and applies mutations only to that copy. Before restart, record file hashes, active show ID, revision, programmer/playback state, and expected output. After restart, wait independently for readiness and bootstrap. Desktop cases launch the packaged app as a child process with unique data and port environment variables and assert process ownership explicitly.

## TIME-001 — Zero tick emits without advancing

**Priority:** P0  
**Primary layer:** Server E2E

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `time-001.show`, and use the active copy for this scenario.

**Detailed procedure:**

1. Start `light-server` with `--test-bench`, call `POST /api/v1/test/clock/reset`, and assert the returned/current time is `2020-01-01T00:00:00Z`.
2. Set programmer fade to 0 for this case. Click fixture 1, set Intensity to 50%, and wait for its programmer revision.
3. Mark Art-Net, sACN, and every subscribed OSC feedback receiver.
4. Call `POST /api/v1/test/clock/advance` with `{"millis":0}` once. Save its response and all packets/messages after the marks.
5. Record new marks and call the identical endpoint/body a second time. Save the second response and packets/messages separately.
6. Compare the two returned timestamps and behavior timestamps while separately checking that protocol sequences advanced once per emitted route frame.

**Assertions:** Both calls report `2020-01-01T00:00:00Z`. Each call emits exactly one current frame per enabled route and one OSC feedback cycle. Protocol sequences increment, while all behavior timestamps remain unchanged.

**Pass condition:** Tests can observe current state repeatedly without introducing application-time movement.

**Implementation status:** Implemented as independent `TIME-001 @api` and `TIME-001 @ui` cases. Both use the same normalized logical-DMX, Art-Net, sACN, OSC-cycle, sequence, and behavior-timestamp oracle; the UI variant sets Prog Fade and enters the fixture value through production controls.

## TIME-002 — Fade boundaries are exact

**Priority:** P0  
**Primary layer:** Rust integration plus selected E2E

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `time-002.show`, and use dimmer fixture 1 in the active copy.

**Detailed procedure:**

1. Set **Prog Fade** to `3.0 s` through the hardware timing summary.
2. Click fixture 1, set Intensity to 0%, and advance 3,000 ms so the starting value is settled.
3. With fixture 1 as a new/open selection, set Intensity to 100% and record that programmer mutation as virtual time 0 for the fade. The encoder target must read 100% immediately, while Fixture Sheet resolved light and DMX remain at the 0% starting value.
4. Record the touch-set value into Cue 1 and verify the stored fixture change has `fade_millis: 3000`. Clear/release the programmer, replay Cue 1, and use the Cue activation as a fresh virtual time 0.
5. Call the clock endpoint with increments that land cumulatively at `0`, `1`, `1499`, `1500`, `1501`, `2999`, and `3000` ms, followed by `3001` to prove the completed value remains stable. The increments are `0, 1, 1498, 1, 1, 1498, 1, 1`; do not pass each checkpoint as though it were an absolute timestamp.
6. At every call, inspect Fixture Sheet, the returned unrounded resolved engine value, the logical slot, and the newest Art-Net/sACN bytes before advancing again.
7. Repeat the touch-set, record, clear, and replay sequence through live Group 3. The Group encoder target must jump immediately, the stored Group Cue change must have `fade_millis: 3000`, and every Group member must follow the same resolved/output boundaries.

**Assertions:** Encoder target values jump immediately. Fixture Sheet, resolved values, logical output, Art-Net, and sACN remain monotonic through the fade; midpoint rounding is documented; the endpoint is exactly 255; the recorded fixture and Group Cue changes retain the 3,000 ms timing; and no wall-time delay changes a checkpoint.

**Pass condition:** Interpolation is reproducible at boundaries and finishes exactly once.

**Implementation status:** Implemented as independent `TIME-002 @api` and `TIME-002 @ui` live-programmer cases plus focused `TIME-002 @ui` fixture-Cue and Group-Cue recording/replay cases. The live cases share one exact boundary oracle. The replay cases additionally assert the immediate encoder target, stored per-change timing, Fixture Sheet resolved display, logical output, and Art-Net/sACN bytes at all eight cumulative checkpoints.

## TIME-003 — Chaser and effect phases survive large jumps

**Priority:** P1  
**Primary layer:** Rust integration

**Starting show:** Load canonical `default-stage.show`, immediately Save As `time-003.show`, and use the active copy. Create a new cue list on fixtures 501.1–501.10 for the chaser and use fixture 501's color emitters for the periodic effect.

**Detailed harness procedure:**

1. Create a Cuelist in `chaser` mode whose ordered steps target heads `501.1` through `501.10`; set an explicit `chaser_step_millis` and record its starting virtual timestamp.
2. Run copy A by advancing exactly one step duration at a time and recording the active step after every tick.
3. From an identical fresh copy B, advance directly across the same number of steps and compare its final step/phase with A.
4. From another fresh copy, advance to the middle of a step, update chaser speed through its Cuelist/speed-group object, then advance to the recalculated boundary.
5. Pause through the playback API, advance several step durations, verify no phase movement, resume, and advance the remaining virtual duration.
6. From an identical start, jump 604,800,000 ms and verify bounded phase calculation rather than scheduler catch-up iterations.
7. Repeat steps 2–6 for a periodic color effect on fixture 501's emitters.
8. **Harness boundary:** the exact phase inspection and one-week jump are deterministic runtime operations rather than operator gestures. The executable `@wire` case drives those seams directly; it does not claim a cosmetic browser equivalent.

**Assertions:** The result is defined by virtual timestamp and configured phase rules, not by the number or duration of scheduler iterations. Large jumps do not create an unbounded catch-up loop.

**Pass condition:** A direct jump and an equivalent series of smaller advances end in the same defined state.

**Implementation status:** Implemented as a supplemental `TIME-003 @wire` harness case for Chaser and Phaser runtime, including speed changes, pause/resume, and the maximum one-week jump. It is intentionally not represented by a cosmetic UI pair: construction and precise phase inspection remain runtime-driven under the explicit harness boundary above.

## SHOW-001 — Save, restart, and reopen

**Priority:** P0  
**Primary layer:** Serial server E2E

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `show-001.show`, and use the active copy for this scenario.

**Detailed procedure:**

1. Click fixtures 5 and 6, press `[REC]`, click Group 3, and choose **Merge**. Confirm Group 3 is `1, 2, 3, 4, 5, 6`.
2. Click Group 3, set Intensity to 40%, press `[REC]`, and click empty Cuelist 1 to record Cue 1. Assign Cuelist 1 to page 1 playback 1 with `[SET]`, the Cuelist 1 pool cell, and the **Assign Cuelist 1** fader target.
3. Press `[CLR]` twice, open Cuelist 1 and inspect the recorded Cue 1, then return to the desk and click **GO +** on assigned page 1 playback 1 so playback remains active. Cuelist View is an editor and does not duplicate playback transport controls.
4. Click fixture 12 and set Intensity to 65% without recording it. Record the durable user's programmer ID and complete state.
5. Open the show menu, click **Save Named Revision**, name it `SHOW-001 before restart`, and confirm. Record the working-copy file hash, active show ID/revision, playback state, and expected output.
6. Call authenticated `POST /api/v1/shutdown` and wait for the exact server PID and port to exit.
7. Start a new `light-server` process with the same temporary data directory and port configuration. Wait separately for `/api/v1/readiness` and `/api/v1/bootstrap`.
8. Reconnect as the same durable user. If the working show is not already active, open `show-001.show` through **Load Latest Autosave**.
9. Inspect Group 3, Cuelist 1, playback 1, the durable programmer, first emitted frame, and repository `light-data` before declaring recovery complete.

**Assertions:** Patch, routes, ordered groups, presets, cues, active show identity, and documented durable programmer/playback state reload correctly. No files appear in repository `light-data`.

**Pass condition:** A normal restart preserves durable state without converting transient values into show data.

**Implementation status:** Implemented first as independent `SHOW-001 @api` and `SHOW-001 @ui` operator workflows with one shared normalized Group/Cue/playback/programmer/revision/output oracle. A supplemental `SHOW-001 @restart` process case adds authenticated graceful shutdown, exact old/new PIDs, named revision and show hash, restored active playback runtime, and the first emitted frame.

## SHOW-002 — Crash during save recovers atomically

**Priority:** P1  
**Primary layer:** Storage integration

**Starting show:** For each injected-failure case, load canonical `compact-rig.show`, immediately Save As `show-002-<case>.show`, and use only that active copy. Save one valid baseline edit before injecting the failure on the next save.

**Detailed harness procedure:**

1. In each fresh working copy, make one valid Group edit, save a named baseline revision, and record every file name, size, hash, and show revision in the temporary data directory.
2. Enable the storage fault point **before atomic replacement**, make a second distinct edit, and invoke the exact save operation. Terminate/restart as required by the injection contract, then open the show and classify it as the complete old or complete new revision.
3. Repeat from a fresh baseline with the fault **during temporary-file write**.
4. Repeat again with the fault **after replacement but before backup cleanup**.
5. For every restart, preserve directory listing, hashes, recovery log, bootstrap diagnostics, and normalized show JSON before cleanup.
6. **Harness boundary:** use the implemented deterministic fixtures for all three named storage replacement points. Killing the process at an uncontrolled wall-time instant does not satisfy any case.

**Assertions:** On restart the server opens either the complete old revision or complete new revision, never truncated or mixed JSON. Recovery and backup choice are logged and exposed to the operator.

**Pass condition:** Interrupted persistence cannot destroy the last known-good show.

**Implementation status:** Implemented as three supplemental `SHOW-002 @restart` harness cases with deterministic SQLite boundary fixtures. Each case first produces complete old and new revisions through the production save path while the server owns the file, stops the server, then stages only the named replacement boundary before restart. The test retains the truncated temporary file or pre-cleanup backup as recovery evidence and accepts only the exact old or new hash. Fault injection has no operator gesture, so a browser pair would not exercise an independent UI adapter.

## SHOW-003 — Invalid active show enters recovery

**Priority:** P0  
**Primary layer:** Server integration

**Starting show:** For each invalid-show case, load canonical `compact-rig.show`, immediately Save As `show-003-<case>.show`, retain an untouched backup of that working copy, and then make the active copy malformed, schema-invalid, or referentially invalid.

**Detailed harness procedure:**

1. For `malformed`, copy the working `.show` file, record its hash, and replace a bounded section with invalid bytes/JSON according to the actual storage format.
2. For `schema-invalid`, produce a syntactically valid show whose required field has the wrong shape. For `referentially-invalid`, retain valid syntax/schema but point one Group or playback at a missing object.
3. Keep an untouched backup outside the active path. Record the active-show metadata that will cause startup to select the invalid copy.
4. Stop the server, place the corrupted fixture at the active path, and start a new process against that data directory.
5. Poll `/api/v1/readiness`, then `/api/v1/bootstrap`; retain logs and diagnostics before opening any replacement show.
6. In the app recovery screen, choose **Load Latest Autosave** for the untouched valid show. The recovery choice uses safe blackout directly and does not require initializing another show or dismissing the recovery state first. Confirm safe output and successful activation.
7. Re-hash the invalid file and prove startup did not overwrite it.

**Operator boundary:** corruption is deliberately performed by the serial file-fixture harness, not through a UI control.

**Assertions:** Server readiness succeeds, output remains safe, bootstrap reports the active-show error, and the operator can select a valid show. The invalid file is not overwritten merely by startup.

**Pass condition:** One bad show cannot prevent the desk from starting or silently erase recovery evidence.

**Implementation status:** Implemented first as independent `SHOW-003 @api` and `SHOW-003 @ui` malformed-show recovery cases using the same valid-show activation, safe-output, and unchanged corrupt-file oracle. Supplemental `SHOW-003 @restart` cases independently cover malformed storage, a wrong-shaped required field, and a syntactically valid missing playback reference, including readiness and bootstrap diagnostics.

## SHOW-004 — Backward-compatible migration is stable

**Priority:** P1  
**Primary layer:** Fixture-based Rust integration

**Starting show:** For each migration case, load canonical `compact-rig.show`, immediately Save As `show-004-<case>.show`, rewrite only the named fields in the active copy into the representative historical shape, and save that derived legacy fixture before testing migration.

**Detailed fixture procedure:**

1. Maintain one generated historical fixture for each supported schema shape. Derive it from a fresh working copy by removing only one matrix row at a time:
   - every `fixture_number` field, whose supported maintained-fixture names restore their documented numbers before deterministic patch-order fallback is considered;
   - Group `color`, `icon`, derivation/freeze metadata, `programming`, `master`, and `playback_fader`, which default to no presentation/derivation/programming, master `1`, and no playback fader;
   - playback layout, activation, x-fade, color, flash-release, swap-protection, and presentation defaults, which are restored according to the playback target;
   - route `destination`, which defaults to `null` so the protocol chooses its standard destination; or
   - a virtual dimmer parameter's physical metadata and capabilities, which default to the linear `0–1` metadata and an empty capability list without changing `virtual_dimmer: true`.
   The persisted legacy Cue row separately removes Cue identities and Cuelist defaults.
2. Record the normalized semantic object graph and hash before loading; do not use an arbitrary old file whose intended values are unknown.
3. Load the fixture through the same Show Store open path used by production and capture migration diagnostics plus the normalized in-memory result.
4. Save the migrated show once, close it, and reopen it. Capture normalized state and bytes.
5. Save, close, and reopen a second time. Assert no second semantic migration or revision-only churn occurs.
6. **Harness only:** historical field removal and byte-level comparison are fixture operations, not operator touches. The optional UI smoke only uses **Load from flash drive** and confirms the migrated show opens.

**Assertions:** Defaults are documented, identities and addressing remain stable, migration is idempotent, and the second open performs no additional semantic rewrite.

**Pass condition:** Supported historical shows migrate once without losing operator intent.

**Implementation status:** Implemented as supplemental `SHOW-004 @restart` Playwright matrix cases for fixture-number, Group, playback, route, virtual-dimmer-metadata, and Cue-identity/default migration. Every case opens through the production Show Store, asserts the documented normalized object, and proves object revision and whole-file byte stability on the second reopen. Rust fixture tests additionally retain exhaustive logical-head repair and built-in default-patch coverage. Raw historical-field removal remains a harness operation rather than a simulated operator gesture.

## DESKTOP-001 — Packaged app owns its child server

**Priority:** P0  
**Primary layer:** macOS packaged smoke

**Starting show:** Load canonical `default-stage.show`, immediately Save As `desktop-001.show` in the packaged app's temporary data directory, and make that working copy the active show before launch.

**Detailed desktop procedure:**

1. Build the packaged Tauri application and locate the exact application bundle/binary under test.
2. Allocate an unused loopback port and a new temporary data directory containing `desktop-001.show` as the active show. Record that no server is listening on the chosen port.
3. Launch the packaged app process with the test data-directory and port environment/arguments used by the Tauri launcher. Record the app PID immediately.
4. Wait for the child-server PID/ownership marker, `/api/v1/readiness`, `/api/v1/bootstrap`, and the frontend-ready marker independently.
5. In the visible WebView, confirm the active show label and that the desk renders.
6. Quit through the app's normal **Quit** action. Wait for the app PID, exact child PID, and loopback listener to disappear within their separate deadlines.

**Assertions:** The app-owned server reaches readiness; the WebView connects, bootstraps a session, and renders the desk; the frontend-ready marker is written; quitting the app terminates its exact child server; the loopback port closes within the process deadline.

**Pass condition:** Packaging, frontend bootstrap, server ownership, and shutdown work without touching normal operator data or an already-running production server.

**Implementation status:** Implemented as Playwright case `DESKTOP-001` by `./test desktop-smoke` against the built `.app`. It seeds the temporary active show through the production server, records app and exact child PIDs, verifies readiness/bootstrap and the frontend marker independently, and proves both the child PID and listener disappear after app exit.

## DESKTOP-002 — Existing server is not adopted as a child

**Priority:** P2  
**Primary layer:** macOS process integration

**Starting show:** Load canonical `default-stage.show`, immediately Save As `desktop-002.show`, and start the independent server with that working copy as its active show.

**Detailed desktop procedure:**

1. Allocate a temporary data directory and loopback port. Start `light-server` directly with `desktop-002.show` active; record its PID and wait for readiness.
2. Launch the packaged app configured to connect to that exact URL. Record the app PID and confirm no second server PID is created for the same task.
3. Wait for the WebView bootstrap and confirm the active show label.
4. Quit through the app's normal **Quit** action and wait only for the app PID to exit.
5. Query `/api/v1/readiness` on the original port, compare the still-running PID with the recorded independent PID, and perform one authenticated write to prove its data directory/log remain usable.
6. Stop the independent server only during test cleanup, after all ownership assertions have passed.

**Assertions:** Record app and server PIDs before quit. The app PID exits, the independent PID remains unchanged, readiness still succeeds, and its data directory and log remain writable by that server.

**Pass condition:** The desktop app terminates only a server process it spawned and never kills an independently owned server.

**Implementation status:** Implemented as Playwright case `DESKTOP-002` by `./test desktop-smoke`. It proves the app creates no child server, exits, leaves the original PID and readiness endpoint intact, and then performs an authenticated show-file write through that same independent process before cleanup.

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
