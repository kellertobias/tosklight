# Test Bench Coverage Catalog

This is the canonical catalog for executable acceptance coverage. Every stable ID listed here is registered in at least one repository-root Playwright test. Stable IDs must be retained when scenarios move between Rust integration tests, the Playwright browser bench, or packaged-desktop smoke tests. Every executable scenario records its actual action surface and oracle so a passing API-only, UI-only, integrated, or wire-protocol case is not mistaken for a different kind of coverage.

[Testing conventions](../../testing/README.md) remain under `docs/testing`, along with the retained OSC/API contract whose incomplete assertions are not yet covered elsewhere. Implemented scenario procedures live with their repository-root Playwright tests. This catalog remains the authoritative ID and coverage index. The short **Actions** entries here are summaries, not manual procedures; do not infer missing operator gestures from them.

The Playwright bench runs an isolated server and data directory per test with a fixed application clock. Advancing virtual time renders and transmits one real output frame, allowing fades, chasers, and effects to be tested without wall-clock waits. Production mode continues to stream at its configured frame rate.

## SHOW-000: copy a reusable show with Save As

The suite retains canonical `compact-rig.show` and `default-stage.show` fixtures instead of regenerating them before each run. Run [SHOW-000](../../../tests/00-generate-show-files.spec.ts) first to prove Save As creates an independent copy without altering its canonical source. Every scenario below loads one canonical file, immediately saves it under the scenario-specific name, and uses only that active working copy.

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
- **Actions:** Use existing Group 3 with fixtures 1–4, apply 50%, add fixtures 5 and 6, subtract fixture 2, then add fixture 2 again and prove it is appended at the end.
- **Oracle:** Group membership and order in the API/UI; Art-Net and sACN slots for current members; removed fixtures retain only independently scoped values.
- **Pass:** Group edits affect group-relative programming immediately; subtraction preserves retained order and a later re-addition appends the removed member.

### DIM-002 — Command-line group programming

- **Starting show:** Load canonical `compact-rig.show`, immediately Save As `dim-002.show`, and use the active copy.
- **Surface:** Lightning Desk keypad/command line.
- **Actions:** Enter `GROUP 1 AT 50`, advance the configured programmer fade exactly to its boundary, and emit one frame.
- **Oracle:** Command-applied audit event, selected live group reference, twelve DMX values of 128, Art-Net universe 1, and sACN universe 101.
- **Pass:** The UI command reaches the engine and both real UDP protocols with identical slot data.

### CMD-001 — Fixture and Group default modes

- **Starting show:** Load canonical `compact-rig.show`, immediately Save As `cmd-001.show`, and use the active copy.
- **Surface:** Lightning Desk keypad/command line plus the versioned command API.
- **Actions:** Toggle the persistent default with `GROUP ENTER`; prove Group mode and `GROUP 3` retain live references; prove a second consecutive Group press replaces `GROUP` with `DEGRP` and dereferences only that term; then mix Fixture and Group additions and ranges.
- **Oracle:** Visible command text and placeholder, persistent default mode, ordered live versus dereferenced source references, normalized deduplicated targets, programmer selection, and command audit.
- **Pass:** Mode toggles, live Group references, scoped dereferencing, and explicit prefixes have deterministic behavior across `+` and `THRU` without changing the default accidentally.

### PROG-002 — Ordered value spreading

- **Starting show:** Load a fresh copy of canonical `compact-rig.show` for every spread case and use Group 1 with ten ordered fixtures.
- **Surface:** Lightning Desk keypad/command line plus engine interpolation and real output.
- **Actions:** Enter uniform `0`, ascending `0 THRU 100`, descending `100 THRU 0`, and multi-point `100 THRU 0 THRU 100` intensity commands; cover both live `GROUP` references and dereferenced `DEGRP`/`GROUP GROUP` fixture captures; reserve empty cases for color and position spreads.
- **Oracle:** Ordered normalized programmer values before quantization, address shape (group-relative spread versus fixture-scoped values), Fixture Sheet values, logical output, and matching Art-Net/sACN slots.
- **Pass:** Intensity spreads preserve target order, endpoints, direction, symmetry, and equal intervals. Live Group spreads recalculate when group membership changes and remain group-relative in Cue/Preset storage; dereferenced spreads stay attached to the captured fixtures. Unresolved center and complex-value rules remain explicitly documented rather than guessed.

### GROUP-003 and GROUP-004 — Derived and frozen membership

- **Starting show:** Load separate canonical `compact-rig.show` copies for the derived and frozen cases and use only their active working copies.
- **Surface:** Lightning Desk command line and Group UI.
- **Actions:** Create an every-second derived group from group 1 and a frozen snapshot of group 1; insert and remove members in group 1.
- **Oracle:** Derived membership recalculates from source order while the frozen group remains unchanged; only genuinely deleted fixture references are reported missing.
- **Pass:** Live/derived and frozen semantics remain distinguishable after source edits.

### PROG-003 — Direct values and group arbitration

- **Starting show:** Load canonical `compact-rig.show`, immediately Save As the `prog-003` working copy, and use the active copy.
- **Surface:** REST programmer API plus UI group programming.
- **Actions:** Put Group 1 at 50%, fixture 1 at 75%, fixture 1 at 25%, repeat Group 1 at 50%, release each source, and then cover second-programmer/playback arbitration in MERGE scenarios.
- **Oracle:** Inside one programmer, the newer fixture or Group programmer value wins by LTP even when the newer value is lower; release falls back to the remaining active source; cue/playback HTP remains covered separately.
- **Pass:** Fixture- and group-scoped programmer contributions resolve by LTP within one programmer, while cross-source HTP/LTP arbitration remains isolated in MERGE coverage.

### PROG-001 — Selection persists through value entry until replaced or cleared

- **Starting show:** Load canonical `compact-rig.show`, immediately Save As `prog-001.show`, and use the active copy.
- **Surface:** Stage clicks and marquee, Fixture Sheet, Group UI, Lightning Desk command/value controls, encoders, and preset recall.
- **Actions:** Select fixtures and groups successively across surfaces, apply repeated values without reselecting, start a replacement selection, use leading `+` to continue a prior selection after a value edit, then press `CLR` once and select again.
- **Oracle:** Current programmer selection state, ordered source references, normalized deduplicated fixture targets, visible selection indicators, leading-plus continuation, and preserved programmer values.
- **Pass:** Selections accumulate across all surfaces as implicit `+` operations, value/encoder/preset edits leave the selection current, the next non-plus selection replaces the old targets, leading `+` continues them, and first-stage `CLR` clears only the selection.

## Default Stage Show

Canonical `default-stage.show` contains the complete 49-record built-in patch:

| Fixture IDs | Name | Capabilities |
| --- | --- | --- |
| 1–6 | Front Fresnels (`1.1`–`1.6`) | Intensity |
| 101–108 | Back Profiles (universe 2 from `2.1`) | Intensity, pan, tilt, RGB color |
| 201–205 | Back LED Washes (universe 2 from `2.49`) | Intensity, pan, tilt, RGB color |
| 401–412 | Floor RGBW PARs (universe 3 from `3.1`) | Intensity and RGBW color |
| 501–506 | Back RGB Sunstrips (universe 3 from `3.61`) | Ten logical RGB heads with virtual dimmers |
| 601–604 | Front RGB Strobes (universe 3 from `3.241`) | Intensity and RGB color |
| 28, 29, 99 | ACL sets (`1.11`, `1.12`) and hazer (`1.13`) | Built-in utility capabilities |
| 301–304 | Back Trackspots (universe 2 from `2.79`) | Movement and intensity |
| 999 | Overhead RGB multipatch (`4.1`) | Intensity and RGB color |

The executable workflows that use this patch are cataloged under the concrete Group, Programmer, Cue, Preload, Move in Black, persistence, and Sound-to-Light IDs below. Earlier draft theater walkthroughs were never implemented as stable-ID scenarios and are not listed as coverage.

## CMD-002 — Set and synchronize speed groups

- **Starting show:** Load canonical `default-stage.show`, immediately Save As `cmd-002-speed-groups.show`, and use the active copy.
- **Surface:** Lightning Desk keypad/command line and Speed Group controls.
- **Actions:** Enter `SHIFT TIME` to display `SPD GRP`; address groups 1–5 as A–E; set integer and decimal-comma BPM values with `AT`; make relative changes with `AT +` and `AT -`; synchronize two groups with `SPD GRP <source> AT SPD GRP <target>`; then break synchronization once by direct BPM entry and once by tap tempo.
- **Oracle:** Exact visible command text, A–E BPM values and precision, active synchronization relationship, and aligned beat phase until the documented break action.
- **Pass:** The shortcut addresses every Speed Group correctly, synchronization copies source speed and phase to the target, and direct entry or tapping either linked group returns the pair to independent operation.
- **Status:** Implemented in the production command surface with server integration and Playwright coverage, including direct-entry and tap-tempo unlinking.

## SOUND-001 — Drive a Speed Group from a desk-local audio input

- **Starting show:** Load canonical `compact-rig.show`, immediately Save As separate `sound-001-api.show` and `sound-001-ui.show` working copies, and begin with Sound-to-Light disabled on Speed Group A.
- **Surface:** Paired authenticated Speed Group API and the Playback Tools Sound-to-Light modal with deterministic Web Audio input.
- **Actions:** Select a browser/desk-local recorded 120 BPM kick input; enable Tempo/BPM analysis; configure a 45–140 Hz band, gain, confidence, smoothing, accepted tempo range, hold, and a 2× ratio; then wait for the server's authoritative Sound source.
- **Oracle:** Exact persisted response configuration without a device ID, browser-local desk/group device mapping, permission/source/signal status, live meters, accepted Sound BPM near 120, and effective Speed Group rate near 240 BPM.
- **Pass:** A reproducible browser analyzer drives the shared authoritative Speed Group while machine-specific input identity remains local and preview cannot publish before Apply.
- **Executable scenario:** [SOUND-001](../../../tests/14-sound-to-light.spec.ts)

## CUE-006 — Select an active playback

- **Starting show:** Load canonical `default-stage.show`, immediately Save As `cue-006-active-playback.show`, and use the active copy.
- **Surface:** Touch/software keyboard, playback controls, command line, and Cuelist details.
- **Actions:** Press Shift-Z to enter `SELECT`, touch page 1 playback 2, run a different playback, open Shift-4 Cue details, and enter `RECORD CUE 7` without an explicit playback or Cuelist Pool address.
- **Oracle:** Exact `SELECT` command text before the playback touch, one explicit active-playback identity, Shift-4 opening that playback's Cuelist, and Cue 7 stored only in the active playback's assigned Cuelist.
- **Pass:** Playback selection is deliberate and remains the shared default for Cue details and address-omitting Cue recording; running another playback does not steal selection, and explicit addresses still take precedence.
- **Status:** Implemented with visible Shift-Z playback selection, desk-and-show scoped persistence, implicit Cuelist resolution, explicit-address override, and Playwright coverage. Sessions attached to the same desk share the selection; another desk used by the same user remains independent.

## DEMO-001 — Narrated planned product demo

- **Starting show:** Load a disposable copy of `default-stage.show`, then create a new empty show through the product-demo application and name it `Demo Show`.
- **Surface:** The real `?demo=product` application, its simulated hardware keypad and playbacks, authenticated show-object setup, the manual clock, and logical DMX observation.
- **Actions:** Add five patch layers; import the exact shipped fixture packages; build and patch the 80-fixture venue, truss, lamp, multipatch, and utility rig; add three output routes; enable Fixture Sheet group shortcuts; record Group 9 through the keypad; apply empty-selection **Lamps On**; create the color, position, and gobo presets; create the main sequence, color looks, group masters, and Speed A ACL chaser; busk the looks; then prepare playback and position changes in Preload with a four-second programmer fade and commit them before learning the chaser speed.
- **Oracle:** Exact active show name; five named layers; 80 persisted patched fixtures; three enabled output routes; named and ordered Group objects; empty-selection programmer lamp/intensity values; preset, Cuelist, playback, page, wrap, chaser, and Speed Group data; enabled playback runtime; pending then committed Preload state; and non-zero authoritative DMX output.
- **Pass:** One Playwright test completes the full operator story without splitting state across scenarios. `./test demo` records the narrated Full HD product surface and writes the maintained screenshot and video below `.artifacts/test/visual-inspection/product-demo/`.
- **Executable scenario:** [DEMO-001](../../../tests/product-demo.spec.ts)

## Required coverage matrix

| IDs | Area | Required cases | Primary oracle |
| --- | --- | --- | --- |
| SHOW-000–005 | Show files and recovery | Save As copy isolation, save/restart/reopen, atomic replacement recovery, invalid-active-show recovery, stable legacy migration, Latest Autosave versus named revisions, timestamped revision-copy provenance, durable copy reopen/restart, Save As provenance preservation, alternate-destination confirmation, and recoverable overwrite | independent files and identities, visible provenance timestamps, hashes/revisions, restarted server, and the contents of preserved backup databases and corrupt files |
| DIM-001–002 | Foundational dimmers | ordered live Group editing and a visible Group command reaching logical, Art-Net, and sACN output | Group/programmer state and exact DMX |
| GROUP-003–005 | Group semantics | derived membership, frozen membership, stored empty Groups, skipped missing range IDs, unpatched members, and safe invalid references | Group objects, selection, visible panes, rendered output |
| PROG-001–004 | Programmer | selection gesture lifetime, ordered spreading, fixture/Group LTP, and two-stage Clear | programmer state, audit, panes, exact output |
| UPDATE-001–002 | Update | four Cue modes and authoritative tracked sources, exact eligibility, Preset/Group existing-versus-new semantics, touch/default/menu flows, current/explicit page context, pre-Update desk-settings migration with a schema-3 show, actual simulator pointer gesture exclusivity, and atomic revisions | paired and process-backed stored-object results, previews, revision history/undo, programmer retention, unrelated-object isolation, restart, simulator transport writes, and same-desk feedback |
| HIGHLIGHT-001 | Transient Highlight output | independent HIGH state, complete/singleton application, programmer/store isolation, fixture-look overrides, first-frame Off reveal, ownership, and safety/master behavior | paired Highlight/programmer state, stored objects, and resolved raw output |
| HIGHLIGHT-002 | Live selection source and lifecycle | Fixtures/Stage/Group/command selection reset, live Group ALL restoration, additive/subtractive stepped selection, multi-head/multipatch/unpatched/invalid items, empty selection, reconnect, and show-load clearing | paired actual selection, live source resolution, Highlight output, reconnect, and reload |
| HIGHLIGHT-003 | Corrected stepping controls, selection visualization, alerts, and surface-specific geometry | real-selection PREV/NEXT/ALL with wrap; independent exact-label HIGH; fixed four-key columns; software-only 2×2 Programmer Fade; subdued Fixture Sheet base and prominent current step including multi-head/collapsed parents; top-layer dismissible errors; no command-bar/simulator Highlight panel; simulator two-row RECORD/Preload and adjacent equal-height Prog/Cue Fade; keyboard/REST/WebSocket/OSC/hardware parity; removed Capture actions | paired programmer/Highlight state, stored Groups, Fixture Sheet rows, production software/simulator geometry, alert reachability, absence of forbidden panels, and protocol responses |
| HIGHLIGHT-004 | Multi-user Highlight ownership | different-user same-desk exclusion, non-mutation on conflict, same-user session retention, last-session release, reacquisition, and other-desk isolation | authenticated session registry, Highlight owner/output state, programmer selection, HTTP conflict, and release lifecycle |
| HIGHLIGHT-005 | Production Highlight alert reachability | ownership and generic action failures at 1280×720 and 1600×1100 in software-only and hardware-connected layouts, above pane/modal content, with pointer and keyboard dismissal and invariant accepted geometry | production browser bounds, hit testing, focus, stacking, exact `HIGH` label, absence of a command-bar status panel, and before/after control geometry |
| HIGHLIGHT-006 | Hardware simulator geometry and faders | rendered RECORD/Preload and fixed key columns, adjacent full-height Programmer/Cue Fade, independent pointer transport, and top/bottom boundaries without a Highlight panel | production hardware-controls CSS/DOM geometry and captured Tauri control writes |
| FIXTURE-001–002 | Fixture profiles | desk-wide atomic revisions, editor/lookup/reorder, exact channels/functions/actions/color/geometry, complete physical metadata, confined photograph/icon/valid-GLB preview and replacement, independent split patching, portable snapshots, GDTF retention, and v1 migration/recovery | paired profile/history, exact asset data, rendered GLB metadata/preview, immutable revisions, raw DMX, patch ranges, Stage geometry, restart snapshots, startup artifacts and warnings |
| FIXTURE-SHEET-001 | Fixture Sheet color boundary | dark, bright, absent, and mixed swatches at 1600×1100 software-only and 1280×720 hardware-connected layouts without changing dot or row geometry | production computed fill, border, dimensions, row bounds, selection styling, and attached pane screenshots |
| MATTER-001–002 | Matter playback bridge | desk-persistent enablement across shows/restart, active-show assignment reconciliation, global page/slot endpoint stability, omissions, OnOff/Level writes, bidirectional tracking, pairing/identity, transport lifecycle, and truthful failures | paired configuration/status, process-backed show/restart state, stable endpoint identity, adapter values, host-network production socket smoke, and subscription state |
| PRELOAD-001–006 | Preload | programmer-only blind values, physical action queue, virtual-playback pane/actions, all eight capture masks, atomic combined GO, programmer-only release | pending entries, one commit timestamp, playback runtime, rendered output |
| VPB-007 | Virtual Playbacks | inert Shift selection and named zone editing; overlap union; concurrent UI/REST/OSC/keyboard activation; hidden membership; restart and desk isolation | persisted zone store, serialized playback runtime, UI Settings, audit, rendered output |
| PBK-001–006 | Playback configuration | Set interception on every playback control, assignment/color/clear persistence, type-safe button and fader layouts, Cuelist actions, Master/X-fade/Temp, Flash/Temp LTP restoration, Swap protection, specialized masters | persisted playback definition, action verb, playback runtime, temporary ownership, master state, exact output |
| CMD-001–002 | Command line | fixture/Group default modes, ranges and dereferencing, plus Speed Group value/synchronization commands | visible command text, programmer state, audit, Speed Group state |
| CUE-001–014 | Cue/playback | record, tracking, cue-only restore, active-Cue deletion with held output/navigation, GO/back, Go To/Load, pause, release, per-value/Cue timing, GO/FOLLOW/TIME triggers, Cuelist View editing and transactional renumbering, Chaser/Speed Group settings, Intensity HTP/LTP, wrapping, First/Continue restart, timing bypass | playback state, persisted Cuelist data, exact virtual timestamps, UI selection without execution |
| SOUND-001 | Sound to Light | desk/browser-local input, permission and source state, frequency/gain/confidence/smoothing/range/hold/ratio configuration, recorded 120 BPM analysis, authoritative mapping, manual fallback and ownership boundaries | persisted Speed Group config, local device mapping, live analysis, authoritative Speed snapshot |
| MIB-001 | Move in Black | per-fixture enable/default, safety delay after resolved zero, future lit-position lookup, disabled comparison, cancellation, Cue-edit invalidation | patch persistence, normalized MIB runtime state, exact Position DMX boundaries |
| MERGE-001–003 | HTP/LTP | programmer priority/recency, programmer/playback arbitration, automatic full-overwrite release, and reversible Flash/Temp ownership | resolved source state and exact DMX |
| DMX-001–008 | Encoding/routes | single-byte values, ArtDMX/E1.31 fields and sequence, remapped/multiple routes, patch boundaries, 16-bit order/defaults, isolated output failure/recovery, minimum payloads, idle configured universes, and disable-without-delete handoff | decoded real UDP datagrams and route diagnostics |
| OSC-001–006 | Hardware OSC | feedback subscription, commands, subscriber isolation, invalid input, same-desk UI/OSC interaction state, desk isolation, and current/explicit page addressing | received OSC messages, command/audit state, UDP output |
| API-001–002 | REST/events | authentication, revision conflicts, CRUD, matching events, and audit ordering | HTTP status/body, events, audit |
| CROSS-001–002 | Cross-surface agreement | equivalent Group value through UI/API/OSC and visible UI synchronization after external mutation | normalized programmer/output state and visible UI |
| TIME-001–003 | Virtual time | zero tick, exact fade boundaries, chaser/phaser speed, pause/resume, and maximum one-week jump | exact virtual timestamp, runtime phase, and output frame |
| DESKTOP-001–002 | Packaged app | WebView load, session/bootstrap, app-owned server readiness and clean child shutdown; independent-server non-adoption and survival | ready marker, HTTP readiness, exact process ownership and exit, authenticated post-exit write |
| FILE-001–002, FILE-016 | File Manager | confined revision-safe text, visible browse/edit, authenticated roots/capabilities/range streaming, file operations, configured roots, pane input ownership, and hosted picker contracts | HTTP status/body, persisted files, visible pane/picker state, OSC-owned input context |
| TEXT-001, TEXT-015 | Text Editor | file association and dirty state, multi-pane synchronization/conflicts, external updates, rename/delete recovery, read-only and Markdown modes | persisted text/layout and visible editor state |
| LOCK-001 | Desk Lock | synchronized multi-screen PIN/button locking, desk-scoped API and OSC suppression, stable output, and other-desk independence | lock API, visible dialogs, command line, DMX, OSC behavior |
| CLIENT-001 | Client history and removal | stable client identity, connected-first presence, last-connected restart persistence, legacy unknown timestamps, duplicate-free reconnect, active/self removal conflicts, scoped confirmed cleanup, and removed-client default re-registration | schema-v9 desk store, live session registry, production Choose default screen UI, process restart, unchanged users/show objects, and new default desk identity |
| DEMO-001 | Planned product demo | empty-show setup, five-layer 80-fixture rig, network routes, Groups, empty-selection Lamps On, Presets, Cues, playback busking, Preload, and Speed A chaser in one narrated run | persisted show objects, visible product-demo controls, programmer/preload/playback state, authoritative DMX, Full HD video, and final screenshot |
| MANUAL-019 | Operator UI review | Desktop/desk terminology, shared modal/window search chrome and stacked options, fixture browser alignment, confined file fields, pane headers, Cue editor composition, Help/Outputs/DMX/Stage responsibilities, diagnostic Development access, and safe recovery load | visible accessible UI, persisted/API state, OSC desk identity, safe-blackout request |

Every catalog entry added later must state setup, action surface, virtual timestamps where relevant, oracle, and pass condition. Protocol scenarios must discard packets captured before their action and assert a newer sequence, preventing stale output from satisfying the test.

## Test implementation rules

Executable tests keep the layers separate:

- Use UI automation for operator actions, visible state, dialogs, and timing controls. Wait for both HTTP bootstrap and the live command WebSocket before interacting.
- Use server or engine tests for exact reference semantics, tracking, persistence, and restart behavior.
- Check rendered fixture values or DMX output for every workflow that claims a live lighting result.
- Give each test its own new show or restore a known fixture so tests cannot pass because another workflow left state behind.
- Preserve the workflow names and pass conditions so automated failures remain understandable to an operator.
- Do not use wall-clock sleeps for lighting behavior. Advance the manual application clock and emit a frame; reserve real time only for browser gestures such as long-press recognition.
- Keep exhaustive interpolation, merge, tracking, and packet-layout combinations in fast Rust tests, with representative Playwright scenarios proving the complete UI/API/OSC-to-UDP path.
- On failure retain the Playwright trace and screenshot plus the server log, audit tail, virtual time, recent OSC messages, and decoded Art-Net/sACN packets.
