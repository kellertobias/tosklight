# Test Bench Coverage Catalog

This is the canonical catalog for automated and manual acceptance coverage. Stable IDs must be retained when scenarios move between Rust integration tests, the Playwright browser bench, or packaged-desktop smoke tests. Every executable scenario records its action surface and oracle so a passing API-only test is never mistaken for UI or wire-protocol coverage.

Detailed scenario specifications are grouped in [docs/testing](../../testing/README.md). The catalog remains the authoritative ID and coverage index; the scenario documents provide the executable setup, actions, timing checkpoints, oracles, and pass conditions.

The Playwright bench runs an isolated server per worker with a fixed application clock. Advancing virtual time renders and transmits one real output frame, allowing fades, chasers, and effects to be tested without wall-clock waits. Production mode continues to stream at its configured frame rate.

## SHOW-000: copy a reusable show with Save As

The suite retains canonical `compact-rig.show` and `default-stage.show` fixtures instead of regenerating them before each run. Run [SHOW-000](../../testing/00-generate-show-files.md) first to prove Save As creates an independent copy without altering its canonical source. Every scenario below loads one canonical file, immediately saves it under the scenario-specific name, and uses only that active working copy.

## Compact Rig: twelve dimmers and four RGB LEDs

Canonical `compact-rig.show` contains fixtures 1–12 as one-channel Generic Dimmers on universe 1, addresses 1–12, on patch layer `Dimmers`. RGB LED fixtures 21–24 use `RGB virtual dimmer` mode at addresses 13, 16, 19, and 22 on patch layer `LEDs`.

| Group | Name | Initial ordered members |
| --- | --- | --- |
| 1 | All Dimmers | 1–12 |
| 2 | Odd Dimmers | 1, 3, 5, 7, 9, 11 |
| 3 | Front Dimmers | 1–4 |
| 4 | Center Spot | Empty |

Configure two enabled routes for logical universe 1: Art-Net universe 1 to the bench Art-Net receiver and unicast sACN universe 101 to the bench sACN receiver. A test that changes routing must restore or replace the show rather than mutating another test's fixture.

### DIM-001 — Create and edit an ordered group

- **Starting show:** Load canonical `compact-rig.show`, immediately Save As `dim-001.show`, and use the active copy.
- **Surface:** REST setup, Lightning Desk UI, and Group UI.
- **Actions:** Use existing group 3 with fixtures 1–4, apply 50%, add fixtures 5 and 6, remove fixture 2, and reorder fixture 6 before fixture 1.
- **Oracle:** Group membership and order in the API/UI; Art-Net and sACN slots for current members; removed fixtures retain only independently scoped values.
- **Pass:** Group edits affect group-relative programming immediately without rewriting unrelated fixture values.

### DIM-002 — Command-line group programming

- **Starting show:** Load canonical `compact-rig.show`, immediately Save As `dim-002.show`, and use the active copy.
- **Surface:** Lightning Desk keypad/command line.
- **Actions:** Enter `GROUP 1 AT 50`, advance the configured programmer fade exactly to its boundary, and emit one frame.
- **Oracle:** Command-applied audit event, selected live group reference, twelve DMX values of 128, Art-Net universe 1, and sACN universe 101.
- **Pass:** The UI command reaches the engine and both real UDP protocols with identical slot data.

### DIM-003 — Frozen and derived membership

- **Starting show:** Load canonical `compact-rig.show`, immediately Save As `dim-003.show`, and use the active copy.
- **Surface:** Lightning Desk command line and Group UI.
- **Actions:** Create an every-second derived group from group 1 and a frozen snapshot of group 1; insert and remove members in group 1.
- **Oracle:** Derived membership recalculates from source order while the frozen group remains unchanged and reports missing fixtures.
- **Pass:** Live/derived and frozen semantics remain distinguishable after source edits.

### DIM-004 — Direct values and group arbitration

- **Starting show:** Load canonical `compact-rig.show`, immediately Save As `dim-004.show`, and use the active copy.
- **Surface:** REST programmer API plus UI group programming.
- **Actions:** Put group 1 at 50%, fixture 1 at 75%, a second programmer at 25%, and vary group master/flash.
- **Oracle:** Exact HTP output is 191 for fixture 1 and 128 for the remaining members; group master and flash scale/restore without moving stored values.
- **Pass:** Fixture- and group-scoped contributions merge predictably and identically in logical DMX, Art-Net, and sACN.

### PROG-001 — Additive selection until use or Clear

- **Starting show:** Load canonical `compact-rig.show`, immediately Save As `prog-001.show`, and use the active copy.
- **Surface:** Stage clicks and marquee, Fixture Sheet, Group UI, Lightning Desk command/value controls, encoders, and preset recall.
- **Actions:** Select fixtures and groups successively across surfaces without programming a value; apply a value; select again; build another selection and recall a preset; then press `CLR` once and select again.
- **Oracle:** Programmer selection open/used state, ordered source references, normalized deduplicated fixture targets, visible selection indicators, and preserved programmer values.
- **Pass:** Selections accumulate across all surfaces as implicit `+` operations until a value/encoder/preset edit consumes the current selection or first-stage `CLR` clears it; the next selection then replaces the old targets and starts a new additive selection.

## Default Stage Show

Canonical `default-stage.show` contains the complete 49-record built-in patch:

| Fixture IDs | Name | Capabilities |
| --- | --- | --- |
| 1–6 | Front Fresnels | Intensity |
| 101–108 | Back Profiles | Intensity, pan, tilt, RGB color |
| 201–205 | Back LED Washes | Intensity, pan, tilt, RGB color |
| 401–412 | Floor RGBW PARs | Intensity and RGBW color |
| 501–506 | Back RGB Sunstrips | Ten logical RGB heads with virtual dimmers |
| 601–604 | Front RGB Strobes | Intensity and RGB color |
| 28, 29, 99, 301–304, 999 | ACL sets, hazer, Trackspots, and RGB multipatch | Built-in utility, movement, and multipatch capabilities |

At the start of each theater workflow, create only the groups it uses from this fresh show copy. The common groups are:

| Group | Name | Members |
| --- | --- | --- |
| 1 | Front Fresnels | 1–6 |
| 2 | Back Profiles | 101–108 |
| 3 | Back LED Washes | 201–205 |
| 4 | All Stage | 1–6, 101–108, 201–205, 401–412, 501–506, 601–604 |

Before each workflow, clear the command line, selection, programmer, preload, and active playbacks unless the workflow says otherwise. Create or load any named presets and cues that a workflow lists as prerequisites; do not rely on accidental state left by an earlier workflow.

## THE-001 — Retain a live group reference

**Starting show:** Load canonical `default-stage.show`, immediately Save As `the-001.show`, use the active copy, and then create common group 2 as listed above.

**Purpose:** Prove that programming stored against a group remains connected to that group's current membership.

1. Select group 2 by reference with `[GRP] 2 [ENTER]`.
   - **Expect:** Back Profiles 101–108 are selected in their group order, and the selection is identified as a live group reference.
2. Set intensity to 60% and choose a visible blue color.
   - **Expect:** All eight profiles show the programmed intensity and color; unrelated fixtures are unchanged.
3. Record the look as cue 1 on an empty playback.
   - **Expect:** The cue stores group-relative changes, not eight copied fixture changes.
4. Clear the programmer and run cue 1.
   - **Expect:** Back Profiles 101–108 reproduce the look from playback.
5. Patch a compatible `ToskLight Built-in / Profile Moving Light / DPTRGB` as fixture 109 on universe 1 at address 361 and add it to group 2 after fixture 108.
   - **Expect:** The group reports nine ordered members without requiring the cue to be rerecorded.
6. Observe the still-running cue, or release and run it again if required by the chosen playback state.
   - **Expect:** Fixture 109 receives the same group-relative intensity and color. Fixtures removed from group 2 stop receiving that group-relative playback data, while unrelated fixture-scoped values remain intact.

**Pass condition:** Membership can change after programming, and the cue follows the live group reference without duplicating or losing fixture-scoped data.

## THE-002 — Compare a derived group with a frozen selection

**Starting show:** Load canonical `default-stage.show`, immediately Save As `the-002.show`, use the active copy, and then create common group 4 as listed above.

**Purpose:** Prove the difference between a live subdivided reference and a static snapshot of group membership.

1. Select every second member of group 4 with `[GRP] 4 [DIV] 2 [ENTER]`.
   - **Expect:** Alternating fixtures are selected using the ordered membership of group 4.
2. Record the selection as group 5, named `All Stage Odd`.
   - **Expect:** Group 5 retains group 4 as its source and retains the every-second derivation rule.
3. Select the current members of group 4 as individual fixtures with `[GRP][GRP] 4 [ENTER]` and record them as group 6, named `All Stage Snapshot`.
   - **Expect:** Group 6 contains a frozen or static ordered membership and does not retain a live membership link to group 4.
4. Add fixture 999 to group 4 in a position where it changes the alternating pattern.
   - **Expect:** Group 5 recalculates from the new group 4 order. Group 6 remains unchanged.
5. Remove one original member of group 4.
   - **Expect:** Group 5 recalculates again. Group 6 still identifies its original member, including a warning if that member is no longer patched.

**Pass condition:** Derived groups track their source and ordering rule, while frozen selections preserve the captured membership until explicitly refreshed.

## THE-003 — Program a short theater scene

**Starting show:** Load canonical `default-stage.show`, immediately Save As `the-003.show`, use the active copy, and then create common groups 1–4 as listed above.

**Purpose:** Exercise a typical theater-programming sequence using reusable presets, tracked cues, and playback.

The scene is a short evening interior: preset, lights up, an actor crosses to center, the room cools, and blackout.

1. Build and record reusable presets:
   - Record a warm color preset for groups 2 and 3; group 1 has intensity only.
   - Record a cool blue color preset for groups 2 and 3.
   - Record a center-stage position preset for group 2.
   - **Expect:** Each preset contains only its intended attribute family and can be recalled on compatible fixtures.
2. Program cue 1, `Preset`:
   - Set all stage intensity to 0 while preparing the warm color and center-stage position values.
   - Record cue 1 on a new playback with a zero-second or deliberately short fade.
   - **Expect:** Running cue 1 establishes the tracked color and position without visible light output.
3. Program cue 2, `Lights Up`:
   - Bring group 1 to 70%, group 2 to 45%, and group 3 to 20% using the warm look.
   - Record cue 2 with a 3-second fade.
   - **Expect:** GO fades from the preset state into the warm stage look over the configured time.
4. Program cue 3, `Cross to Center`:
   - Apply the center-stage position preset to group 2 and raise it to 65%.
   - Record cue 3 with a 2-second fade.
   - **Expect:** Unchanged values from cue 2 track forward; only the intended profile position and intensity change.
5. Program cue 4, `Night`:
   - Apply the cool preset to groups 2 and 3, lower group 1 to 25%, and set group 3 to 50%.
   - Record cue 4 with a 4-second fade.
   - **Expect:** The stage moves smoothly into the cool look while tracked position remains intact.
6. Program cue 5, `Blackout`:
   - Set group 4 intensity to 0 and record cue 5 with a 2-second fade.
   - **Expect:** All stage fixtures fade to zero without deleting their tracked non-intensity values.
7. Clear the programmer, release the playback, and run cues 1–5 using GO.
   - **Expect:** Cue order, current/next indication, fade timing, tracking, Stage view, Fixture Sheet, and rendered output agree throughout the sequence.
8. Use GO minus to return one cue, then GO again.
   - **Expect:** The previous and next looks are reconstructed consistently rather than depending on values left in the programmer.

**Pass condition:** A programmer can build, store, clear, and replay a complete tracked theater sequence with correct preset recall, timing, navigation, and output.

## THE-004 — Prepare a change in Preload without disturbing the live scene

**Starting show:** Load canonical `default-stage.show`, immediately Save As `the-004.show`, use the active copy, create common groups 1–4, and recreate cues 1–5 exactly as described in THE-003 before beginning this scenario.

**Purpose:** Prove that an operator can prepare and store the next change while the audience-facing output remains stable.

1. Run cue 2 from the theater scene workflow and note its live values.
   - **Expect:** The warm `Lights Up` look is active.
2. Enter Preload and select group 3.
   - **Expect:** The UI distinguishes current output, the active preload scene, and pending preload values.
3. Set group 3 to blue at 60% as a pending preload change.
   - **Expect:** The pending values are visible in preload-aware views, but live Stage, Fixture Sheet current values, and rendered DMX do not change.
4. Store the pending preload values into a new cue.
   - **Expect:** The new cue contains the intended group-relative color and intensity while the live cue remains unchanged.
5. Clear pending preload values.
   - **Expect:** Only the pending values clear. The live scene, active preload scene, and stored cue are unchanged.
6. Recreate the pending change and press Preload GO.
   - **Expect:** The change moves into the active preload scene and becomes visible immediately according to Preload GO behavior.
7. Release the preload scene.
   - **Expect:** Preload output is removed cleanly and normal playback output remains authoritative.

**Pass condition:** Pending preload work is isolated from live output, can be stored independently, and can be applied or cleared without corrupting playback state.

## THE-005 — Save, restart, and resume a show

**Starting show:** Load canonical `default-stage.show`, immediately Save As `the-005.show`, use the active copy, create common groups 1–4, and recreate the presets and cues from THE-003 before beginning this scenario.

**Purpose:** Verify the normal end-of-session and recovery path using persisted show data and durable operator state.

1. Save the common test show after creating groups, presets, and the theater Cuelist.
   - **Expect:** The dirty indicator clears only after the persisted show changes are saved.
2. Leave cue 3 active and place a distinct fixture value in the programmer without recording it.
   - **Expect:** Playback and programmer values are visibly distinguishable.
3. Stop and restart the real server and control application, then reconnect as the same user.
   - **Expect:** Startup succeeds without replacing the show, and the same active show opens.
4. Inspect patch, groups, presets, Cuelist, playback position, and programmer.
   - **Expect:** Persisted show objects reload with their ordering and references intact; the durable user's programmer returns; the running cue index and playback state follow the product's documented restart policy.
5. Clear the programmer and run the theater sequence again.
   - **Expect:** Playback output matches the pre-restart show data and no temporary programmer value has been written into a cue.

**Pass condition:** Restart preserves the portable show and durable user data without silently merging transient programmer values into stored programming.

## Required coverage matrix

| IDs | Area | Required cases | Primary oracle |
| --- | --- | --- | --- |
| SHOW-000–006 | Show and patch | Save As copy isolation, create/open/save, addressing, overlap rejection, restart, invalid-show recovery, legacy migration | independent files, API, restarted server, preserved files |
| GROUP-001–008 | Groups | ordered CRUD, add/remove, empty, live, derived, frozen, missing source, nested-cycle rejection | group objects, selection, rendered output |
| PROG-001–008 | Programmer | select, values, fixture override, three-stage clear, undo/redo, preload, masters, two users | programmer state, audit, rendered output |
| CMD-001–010 | Command line | fixture/group ranges, subsets, `AT`, presets, `REC/DEL/MOV/CPY`, cues, page addressing, invalid grammar | UI result, audit event, show object mutation |
| CUE-001–012 | Cue/playback | record, tracking, cue-only restore, GO/back, pause, release, delay/fade, follow/wait, loop/chaser, restart position | playback state at exact virtual timestamps |
| MERGE-001–006 | HTP/LTP | intensity maximum, equal-priority LTP recency, priority override, programmer/playback conflict, release restoration, group/fixture scope | resolved values and exact DMX |
| DMX-001–010 | Encoding/routes | 0/50/75/100%, multi-byte order, disabled/remapped/multiple routes, ArtDMX headers/sequence, E1.31 headers/priority/sequence, termination | decoded real UDP datagrams |
| OSC-001–008 | Hardware OSC | subscribe/unsubscribe, connected state, command keys, faders/buttons, full feedback, invalid alias, multiple desks, subscriber isolation | received OSC messages, audit, UDP output |
| API-001–008 | REST/events | authentication, revision conflict, CRUD, validation failures, WebSocket commands/events, audit ordering, UI/API agreement, shutdown | HTTP status/body, events, audit |
| TIME-001–008 | Virtual time | zero tick, fade boundaries, pause/resume, follow, chaser speed, effect phase, speed change, seven-day jump | exact virtual timestamp and output frame |
| DESKTOP-001 | Packaged app | WebView load, session/bootstrap, app-owned server readiness, clean child shutdown | ready marker, HTTP readiness, process exit |

Every catalog entry added later must state setup, action surface, virtual timestamps where relevant, oracle, and pass condition. Protocol scenarios must discard packets captured before their action and assert a newer sequence, preventing stale output from satisfying the test.

## Test implementation rules

When these definitions become executable tests, keep the layers separate:

- Use UI automation for operator actions, visible state, dialogs, and timing controls. Wait for both HTTP bootstrap and the live command WebSocket before interacting.
- Use server or engine tests for exact reference semantics, tracking, persistence, and restart behavior.
- Check rendered fixture values or DMX output for every workflow that claims a live lighting result.
- Give each test its own new show or restore a known fixture so tests cannot pass because another workflow left state behind.
- Preserve the workflow names and pass conditions so automated failures remain understandable to an operator.
- Do not use wall-clock sleeps for lighting behavior. Advance the manual application clock and emit a frame; reserve real time only for browser gestures such as long-press recognition.
- Keep exhaustive interpolation, merge, tracking, and packet-layout combinations in fast Rust tests, with representative Playwright scenarios proving the complete UI/API/OSC-to-UDP path.
- On failure retain the Playwright trace and screenshot plus the server log, audit tail, virtual time, recent OSC messages, and decoded Art-Net/sACN packets.
