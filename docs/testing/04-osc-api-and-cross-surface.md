# OSC, API, and Cross-Surface Agreement

These scenarios prove that hardware, browser, and API actions use the same application model rather than parallel approximations.

## How to run this file

Before every scenario, load canonical `compact-rig.show`, immediately use Save As with the filename stated by that scenario, and use only the active working copy. Give every OSC client a unique command socket, feedback socket, client ID, and desk alias. Mark OSC and DMX buffers before commands. For API cases, record the current revision and open the WebSocket before mutation. A cross-surface comparison runs in separate fresh tests and compares normalized final state rather than reusing one programmer between surfaces.

OSC scenarios still receive the mandatory `@api` and `@ui` variants for their operator-visible behavior. The `@osc` variant is a third adapter test using the same expected application and DMX state. This distinguishes a server contract failure, a browser adapter failure, and an OSC transport or alias failure.

## OSC-001 — Subscribe and receive deterministic full feedback

**Priority:** P0  
**Primary layer:** UDP E2E

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `osc-001.show`, and use the active copy for this scenario.

**Actions:** Subscribe a hardware client with a valid desk alias and feedback port. Record the receiver mark, then request a zero-duration manual tick.

**Assertions:** Hardware-connected state becomes true. The client receives a complete feedback burst containing the current page, command line, programmer keys, faders, buttons, and playback state. No periodic feedback arrives between manual ticks.

**Pass condition:** Subscription and a manual tick produce one deterministic current-state feedback cycle.

## OSC-002 — Hardware command mutates output and returns feedback

**Priority:** P0  
**Primary layer:** UDP E2E

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `osc-002.show`, and use the active copy for this scenario.

**Actions:** Send the OSC key sequence for `GROUP 1 AT 25`, wait for the programmer mutation, advance 3,000 ms, and emit one frame.

**Assertions:** All twelve dimmer slots equal 64 in real Art-Net and sACN packets. The subscribing client receives updated command/programmer feedback after the action and after the tick.

**Pass condition:** OSC input, shared command parser, application state, OSC return, and network output agree.

## OSC-003 — Subscriber isolation and unsubscribe

**Priority:** P1  
**Primary layer:** UDP E2E

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `osc-003.show`, and use the active copy for this scenario.

**Setup:** Subscribe clients A and B to different valid desk aliases and feedback ports.

**Actions:** Operate desk A, tick, unsubscribe A, operate desk B, and tick again.

**Assertions:** Desk-specific feedback goes only to the appropriate subscriber. After unsubscribe, A receives no further feedback and hardware-connected state remains true until B also unsubscribes.

**Pass condition:** Client identity, desk routing, and connected state are isolated and reference-counted correctly.

## OSC-004 — Invalid OSC input is harmless

**Priority:** P1  
**Primary layer:** UDP/server integration

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `osc-004.show`, and use the active copy for this scenario.

**Cases:** Unknown alias, malformed address, wrong argument type, out-of-range fader, duplicate client ID, unavailable feedback port, and commands from a non-subscribed sender.

**Assertions:** Snapshot revision, programmer state, playback state, subscribers, audit tail, and DMX receiver counts before and after each invalid packet. Only explicitly documented connection bookkeeping may change.

**Pass condition:** Invalid input returns or logs a useful failure where possible, never panics, never mutates unrelated state, and never broadcasts feedback to another client.

## OSC-005 — Current-page and absolute playback addressing

**Priority:** P0

**Primary layer:** UDP/server integration

**Implementation status:** Specified here; do not add the automated test until the OSC test pass is scheduled.

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `osc-005.show`, and use the active copy for this scenario.

**Setup:** Record Cuelist 1 and Cuelist 2 into the Cuelist Pool and first assert that neither recording created a playback-page assignment. Assign Cuelist 1 to page 1 playback 1 and Cuelist 2 to page 2 playback 1. Subscribe desk A with page 1 active and desk B with page 2 active; when exercising multiple screens, give each independently paged screen its own desk alias.

**Actions:** Send `/light/{desk-a}/page-playback/1/button/1` and assert that it operates Cuelist 1. Send the same relative address through desk B and assert that it operates Cuelist 2. Then send `/light/playback/2/1/fader` and assert that it operates page 2 playback 1 regardless of either desk's active page. Change desk A to page 2 and repeat its relative command. Exercise the equivalent button-up and fader values, then repeat the relative cases through the hardware simulator.

**Assertions:** `page-playback` resolves the playback number against only the addressed desk or screen's active page. `/light/playback/{page}/{playback}/...` resolves the explicit global page and playback independently of all current desk pages. Both paths reach the same assigned Cuelist and playback engine state, and canonical feedback uses `page-playback`. An unassigned playback and a Cuelist that exists only in the pool do nothing. The legacy `paged-playback` and direct-playback aliases may be checked separately for compatibility but are not canonical expectations.

**Pass condition:** Relative and absolute OSC addresses select the intended playback across independent page contexts without implicitly assigning Cuelists or exposing groups as playbacks.

## API-001 — Authentication and revision conflict

**Priority:** P0  
**Primary layer:** REST integration

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `api-001.show`, and use the active copy for this scenario.

**Actions:** Attempt protected reads/writes without a token, with an invalid token, with the current revision, and with a stale `If-Match` revision.

**Assertions:** Authentication failures and revision conflicts have stable status codes and bodies. The stale write makes no partial mutation. The successful write increments the revision exactly once.

**Pass condition:** Clients cannot bypass authentication or overwrite newer state silently.

## API-002 — CRUD produces matching events and audit records

**Priority:** P1  
**Primary layer:** REST/WebSocket integration

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `api-002.show`, and use the active copy for this scenario.

**Actions:** Create, rename, and delete a group and a cue while an authenticated event socket is open.

**Assertions:** Each accepted mutation has one strictly increasing revision, one appropriate WebSocket event, and one audit entry containing the acting user/session and object identity.

**Pass condition:** REST response, event stream, audit log, and subsequent reads describe the same mutation order.

## CROSS-001 — Equivalent group value through four surfaces

**Priority:** P0  
**Primary layer:** Representative Playwright E2E

**Starting show:** For each surface variant, load canonical `compact-rig.show`, immediately Save As `cross-001-<surface>.show`, and use only that active copy.

Run the same logical operation in four fresh tests:

1. Click group 1 and set its value in the UI.
2. Enter `GROUP 1 AT 50` through typed/keypad Lightning Desk input.
3. Send the equivalent OSC hardware keys.
4. Submit the equivalent REST programmer command.

After each action, advance 3,000 ms and capture the normalized programmer state, audit event, OSC feedback where applicable, and real UDP packets.

**Assertions:** Compare group ID, attribute, normalized value, edit semantics, twelve Art-Net bytes, and twelve sACN bytes against one shared expected object. Assert each surface produces exactly one accepted mutation.

**Pass condition:** All four surfaces produce the same group-scoped programmer value and twelve output bytes of 128; only surface-specific metadata differs.

## CROSS-002 — UI and API agree after an external mutation

**Priority:** P1  
**Primary layer:** Playwright E2E

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `cross-002.show`, and use the active copy for this scenario.

**Actions:** Keep the Fixtures and Groups UI open, mutate group membership and programmer values through REST, and wait for revision/WebSocket evidence rather than reloading.

**Assertions:** Visible membership, selection, source indicators, values, and output update to the new revision. Refreshing the page does not change the result.

**Pass condition:** The browser is a live view of server state and does not rely on optimistic local state after external changes.

## Follow-ups

| Scenario | Next tests after the primary case | First failure checks |
| --- | --- | --- |
| OSC-001 | Verify incremental feedback after the initial full burst and after page changes. | Decode raw OSC address/type tags and compare subscriber target/alias. |
| OSC-002 | Repeat with faders, playback buttons, and key-up/key-down semantics. | Compare received key event, parsed command, programmer mutation, then UDP output. |
| OSC-003 | Reuse a client ID, disconnect without unsubscribe, and test expiry policy. | Inspect subscriber registry and per-target feedback logs. |
| OSC-004 | Fuzz address and argument shapes within bounded packet sizes. | Confirm no revision/audit mutation and retain malformed raw packet bytes. |
| OSC-005 | Add three independent screen aliases, sparse page assignments, unassigned Cuelists, and legacy-alias compatibility. | Compare each alias's active page, resolved page/slot assignment, selected Cuelist, and emitted canonical feedback address. |
| API-001 | Test concurrent writers and token invalidation after session shutdown. | Compare pre/post object and revision; failed writes must be byte-for-byte unchanged. |
| API-002 | Reconnect the event socket and request audit from the last seen revision. | Compare response revision, broadcast revision, and audit revision. |
| CROSS-001 | Add fixture selection, cue GO, and group membership as further equivalence families. | Normalize surface-specific metadata before diffing application state. |
| CROSS-002 | Disconnect/reconnect WebSocket and mutate while offline. | Determine whether divergence begins in event delivery, refetch, or UI rendering. |
