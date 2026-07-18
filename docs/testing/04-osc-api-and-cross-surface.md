# OSC, API, and Cross-Surface Agreement

These scenarios prove that hardware, browser, and API actions use the same application model rather than parallel approximations.

## How to run this file

Before every scenario, load canonical `compact-rig.show`, immediately use Save As with the filename stated by that scenario, and use only the active working copy. Give every OSC client a unique command socket, feedback socket, and client ID. Attach the client to the same desk alias as its Tauri application whenever the hardware is meant to extend that physical desk; use a different alias only when the scenario is proving desk isolation. Mark OSC and DMX buffers before commands. For API cases, record the current revision and open the WebSocket before mutation. A cross-surface comparison runs in separate fresh tests and compares normalized final state rather than reusing one programmer between surfaces.

A **desk** in these tests is one combined operator surface: a Tauri application plus every OSC controller, such as an Arduino, subscribed to that application's desk alias. Those inputs share one authoritative page, command line, button state, and playback interaction state. A physical OSC button acts exactly like the corresponding button in that desk's UI. A second Tauri application with a different desk alias is a separate desk and may have its own OSC controller without leaking partially entered commands or page changes across desks. Programmer values are not desk-owned: after a completed command lands them in the user's programmer, the same user's sessions see them on every desk.

“Programmer” and “desk” are intentionally different scopes. Programmer values belong to the user, so once a completed command lands a value in the programmer, every session for that user sees the same value layer. An unfinished command, key/button state, and active page belong to the desk. Two desks logged in as the same user therefore share landed programmer values but retain independent partial command lines; OSC joins the interaction state of the desk alias to which it subscribed.

OSC scenarios still receive the mandatory `@api` and `@ui` variants for their operator-visible behavior. The `@osc` variant is a third adapter test using the same expected application and DMX state. This distinguishes a server contract failure, a browser adapter failure, and an OSC transport or alias failure.

## OSC-001 — Subscribe and receive deterministic full feedback

**Priority:** P0  
**Primary layer:** UDP E2E

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `osc-001.show`, and use the active copy for this scenario.

**Detailed procedure:**

1. Bind separate UDP command and feedback sockets for client `osc-001-a`; record the chosen feedback port.
2. Send `/light/subscribe` with OSC arguments `("osc-001-a", "main", <feedback-port>)` from the command socket.
3. Wait for `/light/main/feedback/page`; this is evidence that subscription succeeded and identifies the exact desk alias used for all later addresses.
4. Record the feedback-message mark after the initial subscription burst.
5. Call `POST /api/v1/test/clock/advance` with `{"millis":0}` exactly once.
6. Collect messages after the mark and verify one complete feedback cycle. Then leave virtual time unchanged for a bounded wall-time window and prove no unsolicited periodic cycle arrives.

**Assertions:** Hardware-connected state becomes true. The client receives a complete feedback burst containing the current page, command line, programmer keys, faders, buttons, and playback state. No periodic feedback arrives between manual ticks.

**Pass condition:** Subscription and a manual tick produce one deterministic current-state feedback cycle.

## OSC-002 — Hardware command mutates output and returns feedback

**Priority:** P0  
**Primary layer:** UDP E2E

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `osc-002.show`, and use the active copy for this scenario.

**Detailed procedure:**

1. Subscribe client `osc-002-a` to desk alias `main` exactly as in OSC-001. Mark OSC, Art-Net, and sACN receivers.
2. Send these OSC messages from the subscribed command socket, in order, with a pressed argument of `true`: `/light/main/programmer/group`, `/light/main/programmer/digit-1`, `/light/main/programmer/at`, `/light/main/programmer/digit-2`, `/light/main/programmer/digit-5`, `/light/main/programmer/enter`.
3. Do not send key-up messages unless the hardware adapter explicitly requires them; the programmer handler acts on pressed messages.
4. Wait until the programmer contains Group 1 intensity `0.25` and feedback shows the resulting cleared/updated command line.
5. Call the virtual-clock advance endpoint with `{"millis":3000}` and inspect the returned frame plus the first newer Art-Net and sACN packets.

**Assertions:** All twelve dimmer slots equal 64 in real Art-Net and sACN packets. The subscribing client receives updated command/programmer feedback after the action and after the tick.

**Pass condition:** OSC input, shared command parser, application state, OSC return, and network output agree.

## OSC-003 — Subscriber isolation and unsubscribe

**Priority:** P1  
**Primary layer:** UDP E2E

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `osc-003.show`, and use the active copy for this scenario.

**Detailed procedure:**

1. Bind independent command/feedback socket pairs for clients `osc-003-a` and `osc-003-b`.
2. Send `/light/subscribe` as `("osc-003-a", "main", <A-feedback-port>)` and `("osc-003-b", <second-valid-desk-alias>, <B-feedback-port>)`. Wait for each alias's feedback/page address.
3. Mark both feedback buffers. Send one programmer key from A's command socket, call a 0 ms tick, and verify only A receives the corresponding desk-specific command feedback while B receives only its own normal tick feedback.
4. Send `/light/unsubscribe` with `("osc-003-a")` from A. Record fresh marks for both clients.
5. Send a different programmer key from B, call a 0 ms tick, and verify A receives nothing while B receives its update.
6. Read bootstrap and confirm `hardware_connected` remains true. Unsubscribe B, tick once more, and confirm it becomes false.

**Assertions:** Desk-specific feedback goes only to the appropriate subscriber. After unsubscribe, A receives no further feedback and hardware-connected state remains true until B also unsubscribes.

**Pass condition:** Client identity, desk routing, and connected state are isolated and reference-counted correctly.

## OSC-004 — Invalid OSC input is harmless

**Priority:** P1  
**Primary layer:** UDP/server integration

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `osc-004.show`, and use the active copy for this scenario.

**Detailed cases:**

1. Before every invalid packet, record snapshot revision, programmers, playbacks, subscriber state, audit tail, OSC marks, and DMX marks.
2. Send `/light/subscribe` with a nonexistent desk alias; then send one with a string where the feedback-port integer belongs.
3. Send malformed/noncanonical addresses including a missing action segment and an unknown programmer key.
4. From a valid subscriber, send a playback fader value below 0 and above 1, once as float and once with the wrong string argument type.
5. Attempt to reuse one client ID from a different command socket and feedback port. Assert the documented replace/reject policy explicitly.
6. Subscribe to a feedback port that is not bound, then tick; UDP unreachability must not mutate programmer/playback state.
7. From a command socket that never subscribed, send `/light/main/programmer/digit-1` and `/light/main/programmer/enter`.
8. After each individual case, reread all recorded state and run a bounded packet/message check before moving to the next packet.

**Assertions:** Snapshot revision, programmer state, playback state, subscribers, audit tail, and DMX receiver counts before and after each invalid packet. Only explicitly documented connection bookkeeping may change.

**Pass condition:** Invalid input returns or logs a useful failure where possible, never panics, never mutates unrelated state, and never broadcasts feedback to another client.

## OSC-005 — Tauri UI and OSC hardware form one desk

**Priority:** P0

**Primary layer:** Tauri/UDP E2E

**Implementation status:** Implemented by paired API/UI coverage plus a two-browser, two-alias UDP Playwright scenario. The executable case interleaves UI and OSC keys, proves same-user values are shared while both desks retain independent partial commands, exercises Fixture and Group defaults, verifies one applied-command audit event and DMX result, reconnects to the original desk, then reattaches the same hardware client to the other alias.

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `osc-005.show`, and use the active copy for this scenario. Ensure Groups 7 and 1 exist and that fixtures 8 and 2 are patched so mixed Group/Fixture terms are distinguishable.

**Detailed procedure:**

1. Start Tauri application A as desk A and subscribe Arduino/OSC client A to desk A's alias. Start Tauri application B as desk B with a different alias and subscribe Arduino/OSC client B to desk B's alias.
2. With Tauri A in its default Fixture mode, press `[GRP] [7] [+]`. Do not press `[ENTER]`. Verify A's visible command line and A's OSC command-line feedback both show `G7 +`.
3. From OSC client A, press only the physical `8` button, sending `/light/{desk-a}/programmer/digit-8` with pressed `true`. Verify Tauri A immediately shows `G7 + F8`, exactly as pressing the on-screen `8` in Fixture mode would; it must not create a second hidden OSC-only command line or replace the partial UI command.
4. Continue from either surface with `[AT] [5] [0] [ENTER]`. Verify the one combined command applies 50% to Group 7 and fixture 8, clears or restores the command line consistently on Tauri A and OSC feedback, and produces one programmer mutation.
5. Throughout steps 2–4, verify Tauri B and OSC client B retain their own unchanged partial command line and page. If both desks use the same user, the completed value from step 4 appears in that user's shared programmer on B as well; it must not alter B's unfinished desk command.
6. Start simultaneous partial commands in Fixture mode: enter `[GRP] [7] [+]` in Tauri A and `[GRP] [1] [+]` in Tauri B. Press physical `2` on OSC client B and verify only desk B becomes `G1 + F2` while A remains `G7 +`. Then press physical `8` on OSC client A and verify only desk A becomes `G7 + F8` while B remains `G1 + F2`. Repeat once after toggling one desk to Group default mode and prove its bare physical digit uses `G`, matching that desk's UI default without affecting the other desk.
7. Disconnect and reconnect OSC client A to desk A's alias. Verify the initial feedback burst restores A's current page, command line, and programmer state. Reconnect it intentionally to desk B's alias and prove subsequent input joins desk B instead; association is determined by the subscribed desk alias, not by hardware identity or source IP.

**Assertions:** UI key presses and OSC key presses addressed to one desk alias are serialized through the same command-line state machine and are visible on both surfaces after every key. Consecutive ordinary fixture/Group selections and their ordered source references are part of that desk-local interaction state until a value is confirmed. A completed mixed-surface command produces exactly one authoritative programmer mutation and one resulting output state. Different desk aliases isolate partial commands, open selection gestures, page selection, and feedback even when both Tauri applications use the same light server. Sessions for the same user share the landed programmer value without sharing those desk-local interaction states.

**Pass condition:** Each Tauri application and its attached OSC hardware behave as one physical light-control desk, while a second application and its hardware behave as an independent desk.

## OSC-006 — Current-page and explicit-page playback addressing

**Priority:** P0

**Primary layer:** UDP/server integration

**Implementation status:** Implemented by paired API/UI coverage plus a two-desk UDP Playwright scenario. The executable case changes the first desk's page through the hardware-connected Tauri page picker, verifies canonical page feedback, retargets a byte-identical current-page command, proves button-up is inert, and exercises OSC float values `0.0`, `0.5`, and `1.0` through both current-page and explicit-page fader forms.

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `osc-006.show`, and use the active copy for this scenario.

**Detailed procedure:**

1. Treat **current-page playback 1** as the OSC form that names playback 1 but no page (`/light/{desk}/page-playback/1/...`), and **explicit-page playback 1** as the form that names both page and playback (`/light/playback/{page}/1/...`). This terminology is mandatory in the test report; do not use “paged” and “non-paged” without stating which address form is meant.
2. Program any visible value, press `[REC]`, and click empty Cuelist 1. Clear twice, program a distinguishable value, press `[REC]`, and click empty Cuelist 2.
3. Read playback pages and prove neither Cuelist was assigned by recording. Press `[SET]`, click Cuelist 1, and touch page 1 playback fader 1. Change the desk to page 2, press `[SET]`, click Cuelist 2, and touch page 2 playback fader 1.
4. Configure two Tauri control desks with distinct valid aliases. In Tauri A select page 1; in Tauri B select page 2. Subscribe one OSC socket pair to each matching alias and wait until feedback confirms those pages.
5. From A, send `/light/{desk-a}/page-playback/1/button/1` with `true`; verify Cuelist 1 advances. Send the exact same playback-1 address from B and verify Cuelist 2 advances because B is on page 2.
6. Change desk A from page 1 to page 2 using the page control in Tauri A, not an OSC page command. Wait until A's OSC feedback reports page 2, then resend the byte-for-byte identical `/light/{desk-a}/page-playback/1/button/1` command. Verify it now reaches Cuelist 2. Desk B's page and state must remain unchanged.
7. Change Tauri A back to page 1. Send `/light/playback/2/1/fader` with `0.5` from A and verify explicit page 2 playback 1 becomes 50% even though A is currently displaying page 1. Repeat from B and verify the same global playback is addressed independently of B's current page.
8. For each button, send the matching `false` button-up and prove it does not create a second GO. Exercise fader values `0.0`, `0.5`, and `1.0` through both current-page and explicit-page forms.
9. Repeat the current-page cases through the hardware simulator using its actual Tauri page control and physical playback-1 control, not direct OSC injection.

**Assertions:** `page-playback` resolves the playback number against only the addressed desk or screen's active page, including a page selected in that desk's Tauri UI. Repeating the same current-page playback-1 OSC packet after a UI page change reaches the playback-1 assignment on the new page. `/light/playback/{page}/{playback}/...` resolves the explicit global page and playback independently of all current desk pages. Both paths reach the same assigned Cuelist and playback engine state, and canonical feedback uses `page-playback`. An unassigned playback and a Cuelist that exists only in the pool do nothing. The legacy `paged-playback` and direct-playback aliases may be checked separately for compatibility but are not canonical expectations.

**Pass condition:** Relative and absolute OSC addresses select the intended playback across independent page contexts without implicitly assigning Cuelists or exposing groups as playbacks.

## API-001 — Authentication and revision conflict

**Priority:** P0  
**Primary layer:** REST integration

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `api-001.show`, and use the active copy for this scenario.

**Detailed procedure:**

1. Create a session with `POST /api/v1/sessions`, retain its bearer token, and GET Group 3 to record its current body and revision.
2. Repeat the protected GET with no `Authorization` header and then with `Authorization: Bearer invalid`; record both status/body pairs.
3. PUT one deliberate Group 3 membership change with the valid bearer token and `If-Match: <current-revision>`. Verify success and capture the new revision.
4. PUT a different body while reusing the old `If-Match` value. Expect a revision conflict.
5. GET Group 3 again and byte-compare it with the successful write; the stale body must not appear anywhere.

**Assertions:** Authentication failures and revision conflicts have stable status codes and bodies. The stale write makes no partial mutation. The successful write increments the revision exactly once.

**Pass condition:** Clients cannot bypass authentication or overwrite newer state silently.

## API-002 — CRUD produces matching events and audit records

**Priority:** P1  
**Primary layer:** REST/WebSocket integration

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `api-002.show`, and use the active copy for this scenario.

**Detailed procedure:**

1. Authenticate, open `/api/v1/events` with the token subprotocol, and record the current show revision and audit tail.
2. PUT a new Group object at an unused ID with expected revision 0. Wait for its WebSocket event and audit entry before continuing.
3. PUT the same Group with a new `name` and the returned current revision. Wait for the rename event/audit entry.
4. DELETE the Group with its current revision and wait for the delete event/audit entry.
5. Repeat the same create, rename, and delete sequence for a Cuelist/Cue object at an unused ID, using each returned revision as the next `If-Match` value.
6. GET both object kinds after every accepted mutation and compare response revision, event revision, and audit revision before issuing the next write.

**Assertions:** Each accepted mutation has one strictly increasing revision, one appropriate WebSocket event, and one audit entry containing the acting user/session and object identity.

**Pass condition:** REST response, event stream, audit log, and subsequent reads describe the same mutation order.

## CROSS-001 — Equivalent group value through four surfaces

**Priority:** P0  
**Primary layer:** Representative Playwright E2E

**Starting show:** For each surface variant, load canonical `compact-rig.show`, immediately Save As `cross-001-<surface>.show`, and use only that active copy.

Run the same logical operation in four fresh tests:

1. **Direct UI:** open Groups, click Group 1 once, touch Intensity, enter `50`, and confirm it.
2. **Lightning Desk keypad:** press `[GRP] [1] [AT] [5] [0] [ENTER]` on the rendered keypad.
3. **OSC hardware:** subscribe to `main`, then send pressed messages for `group`, `digit-1`, `at`, `digit-5`, `digit-0`, and `enter` at `/light/main/programmer/<key>`.
4. **API:** send authenticated command `programmer.group.set` with `{"group_id":"1","attribute":"intensity","value":0.5}`.

Before each variant's action, mark its event/OSC/UDP observers. After the accepted programmer mutation, call the virtual-clock endpoint with `{"millis":3000}` and capture normalized programmer state, audit event, OSC feedback where applicable, and the first newer real UDP packets.

**Assertions:** Compare group ID, attribute, normalized value, edit semantics, twelve Art-Net bytes, and twelve sACN bytes against one shared expected object. Assert each surface produces exactly one accepted mutation.

**Pass condition:** All four surfaces produce the same group-scoped programmer value and twelve output bytes of 128; only surface-specific metadata differs.

## CROSS-002 — UI and API agree after an external mutation

**Priority:** P1  
**Primary layer:** Playwright E2E

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `cross-002.show`, and use the active copy for this scenario.

**Detailed procedure:**

1. Open a layout containing both Fixture Sheet and Groups. Open Group 3's long-press/context view so its ordered members are visible. Record the browser's last applied revision.
2. Through REST, GET Group 3, append fixture 5 to its `fixtures` array, and PUT the complete object with the current `If-Match` revision.
3. Wait for the matching WebSocket/object revision in the browser. Without reloading, confirm Group 3 reports five members in order and the context view includes fixture 5.
4. Through the authenticated command WebSocket, submit `programmer.group.set` for Group 3 intensity 50%.
5. Wait for the programmer event. Confirm the visible Group source/value indicators update, then advance 3,000 ms and inspect output.
6. Reload the browser only after all live assertions pass; confirm the post-reload view is identical.

**Assertions:** Visible membership, selection, source indicators, values, and output update to the new revision. Refreshing the page does not change the result.

**Pass condition:** The browser is a live view of server state and does not rely on optimistic local state after external changes.

## Highlight and Step Through boundary coverage

Focused server, fixture, engine, and control-surface tests cover Highlight without inventing a client-local step model:

- the server registry keeps actual programmer selection, remembered live step source, and HIGH active state independent; NEXT/PREV change the real selection and wrap, ALL re-resolves the live source, external selections replace the basis, and programmer-value changes do not;
- live Group restoration, additive/subtractive selection while stepped, multi-head expansion, logical multipatch identity, authoritative order, duplicate handling, invalid-member removal, unpatched participation, and empty-sequence behavior remain deterministic;
- a different user cannot take over live output on the same desk, while Blind/Preview/Preload can retain the authoritative selection and HIGH state with output suppressed;
- the OSC adapter accepts `on`, `off`, `toggle`, `next`, `previous`/`prev`, and `all`, rejects removed `capture` and `reset` inputs, rejects an identical repeat strictly inside 150 ms, accepts a different action immediately, and accepts the same action at the exact boundary;
- fixture migration defaults a missing per-instance override map to empty, round-trips overrides keyed by stable channel ID, and profile resolution applies an override at the channel's exact raw resolution;
- engine coverage proves the transient raw look remains outside programmer values, clearing it restores normal rendering, and Group Master, Grand Master, and Blackout remain safety limits;
- software and hardware controls cover the exact HIGH/PREV/NEXT/ALL row, `Alt+H`/`Alt+Left`/`Alt+Right`/`Alt+A`, removed `Alt+C`, HIGH's exact label and active treatment, output suppression, and independent complete/step/index/total/fixture protocol feedback;
- the Fixture Sheet covers subdued remembered-base rows, a prominent current step, multi-head rows, master-only contained state when subheads are hidden, HIGH-independent presentation, ALL restoration, and external-selection reset;
- software layout coverage measures the two-column-by-two-row Programmer Fade, while simulator coverage measures two-row RECORD/PRELOAD GO, equal adjacent full-height Programmer/Cue Fade faders, and the absence of a dedicated Highlight display; and
- both main-desk layouts cover a body-level dismissible Highlight alert above panes and modal surfaces, unchanged HIGH/grid geometry, and the absence of a command-bar Highlight status panel.

Run both focused server targets with `cargo test -p light-server --no-default-features --lib highlight::` for the authoritative registry and `cargo test -p light-server --no-default-features --bin light-server highlight` for REST, OSC feedback/deduplication, authenticated reconnect, Blind/Preview/Preload, and session cleanup. Then run `cargo test -p light-fixture highlight` and `cargo test -p light-engine transient_highlight`. From `apps/control-ui`, run the Highlight controls, Fixture Sheet Highlight, command-line keyboard, num-block layout, hardware-connected alert, and attached-hardware layout suites; type-check and build the hardware simulator as well. Paired `HIGHLIGHT-001` covers transient output/programmer isolation, paired `HIGHLIGHT-002` covers live sources and selection/lifecycle boundaries, and paired `HIGHLIGHT-003` proves the corrected actions, selection visualization, alerts, forbidden-panel absence, and surface-specific geometry in [the executable Update, Highlight, Fixture Profiles, and Matter scenarios](../../tests/11-update-highlight-fixture-profiles-and-matter.spec.ts).

`PATCH-001` covers the Show Patch Stage preview boundary. The title-bar action opens a lower-right 16:9 overlay no wider or taller than half the Patch window; selected single, additive, subtractive, and Shift-range fixtures illuminate virtually while retaining live direction and other resolved attributes. With enough fixtures to fill the table, the final row scrolls fully above the overlay. The desk configuration defaults physical Patch preview Highlight off, legacy configuration without the field loads false, enabling it applies only the Patch preview selection's configured Highlight Looks, and closing the preview/session/show or disabling the option releases that contribution without changing the independent HIGH state.

## Matter playback bridge boundary coverage

Matter uses stable explicit page/playback endpoints rather than an OSC subscriber's current page. Focused Rust tests cover endpoint derivation, empty and non-fader omission, On/Off and Level writes, tracking feedback, truthful commissioning status, pairing-identity persistence, and endpoint removal without renumbering surviving lights. The physical-desk Matter settings component tests cover the persisted enable toggle, running and failed status, zero exposed lights for empty slots, pairing code and QR payload presentation, copy action, and the separate count for assigned controls without a dimmable fader.

Run both deterministic server layers with `cargo test -p light-server --no-default-features matter::` for the adapter/transport module and `cargo test -p light-server --no-default-features tests::matter_` for the main server dispatcher/runtime integration tests, then run `npm test -- DeskSettingsModal.test.tsx` from `apps/control-ui`. Paired `MATTER-001` root coverage proves the persisted API/UI toggle and global endpoint identities. The socket-level smoke is deliberately opt-in because it binds UDP 5540 and shared mDNS 5353: run `cargo test -p light-server --no-default-features matter::transport::tests::commissionable_network_transport_smoke -- --ignored --test-threads=1`. That smoke proves host networking, commissioning-window advertisement, value reconciliation, endpoint removal/restart, stable identity, and shutdown. A real controller remains the final interoperability check for fabric commissioning and subscription behavior; the automated suite does not claim that a browser or mock is an independent Matter controller.

## Follow-ups

| Scenario | Next tests after the primary case | First failure checks |
| --- | --- | --- |
| OSC-001 | Verify incremental feedback after the initial full burst and after page changes. | Decode raw OSC address/type tags and compare subscriber target/alias. |
| OSC-002 | Repeat with faders, playback buttons, and key-up/key-down semantics. | Compare received key event, parsed command, programmer mutation, then UDP output. |
| OSC-003 | Reuse a client ID, disconnect without unsubscribe, and test expiry policy. | Inspect subscriber registry and per-target feedback logs. |
| OSC-004 | Fuzz address and argument shapes within bounded packet sizes. | Confirm no revision/audit mutation and retain malformed raw packet bytes. |
| OSC-005 | Interleave every keypad token across UI and OSC, reconnect mid-command, and attach multiple hardware clients to one desk alias. | Compare the authoritative command line after every key, the acting desk/session identity, mutation count, and feedback recipients. |
| OSC-006 | Add three independent screen aliases, sparse page assignments, unassigned Cuelists, and legacy-alias compatibility. | Compare each alias's active page, resolved page/slot assignment, selected Cuelist, and emitted canonical feedback address. |
| API-001 | Test concurrent writers and token invalidation after session shutdown. | Compare pre/post object and revision; failed writes must be byte-for-byte unchanged. |
| API-002 | Reconnect the event socket and request audit from the last seen revision. | Compare response revision, broadcast revision, and audit revision. |
| CROSS-001 | Add fixture selection, cue GO, and group membership as further equivalence families. | Normalize surface-specific metadata before diffing application state. |
| CROSS-002 | Disconnect/reconnect WebSocket and mutate while offline. | Determine whether divergence begins in event delivery, refetch, or UI rendering. |
