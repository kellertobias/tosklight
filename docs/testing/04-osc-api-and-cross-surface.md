# OSC, API, and Cross-Surface Agreement

These scenarios prove that hardware, browser, and API actions use the same application model rather than parallel approximations.

## How to run this file

Give every OSC client a unique command socket, feedback socket, client ID, and desk alias. Mark OSC and DMX buffers before commands. For API cases, record the current revision and open the WebSocket before mutation. A cross-surface comparison runs in separate fresh tests and compares normalized final state rather than reusing one programmer between surfaces.

## OSC-001 — Subscribe and receive deterministic full feedback

**Priority:** P0  
**Primary layer:** UDP E2E

**Actions:** Subscribe a hardware client with a valid desk alias and feedback port. Record the receiver mark, then request a zero-duration manual tick.

**Assertions:** Hardware-connected state becomes true. The client receives a complete feedback burst containing the current page, command line, programmer keys, faders, buttons, and playback state. No periodic feedback arrives between manual ticks.

**Pass condition:** Subscription and a manual tick produce one deterministic current-state feedback cycle.

## OSC-002 — Hardware command mutates output and returns feedback

**Priority:** P0  
**Primary layer:** UDP E2E

**Actions:** Send the OSC key sequence for `GROUP 1 AT 25`, wait for the programmer mutation, advance 3,000 ms, and emit one frame.

**Assertions:** All twelve dimmer slots equal 64 in real Art-Net and sACN packets. The subscribing client receives updated command/programmer feedback after the action and after the tick.

**Pass condition:** OSC input, shared command parser, application state, OSC return, and network output agree.

## OSC-003 — Subscriber isolation and unsubscribe

**Priority:** P1  
**Primary layer:** UDP E2E

**Setup:** Subscribe clients A and B to different valid desk aliases and feedback ports.

**Actions:** Operate desk A, tick, unsubscribe A, operate desk B, and tick again.

**Assertions:** Desk-specific feedback goes only to the appropriate subscriber. After unsubscribe, A receives no further feedback and hardware-connected state remains true until B also unsubscribes.

**Pass condition:** Client identity, desk routing, and connected state are isolated and reference-counted correctly.

## OSC-004 — Invalid OSC input is harmless

**Priority:** P1  
**Primary layer:** UDP/server integration

**Cases:** Unknown alias, malformed address, wrong argument type, out-of-range fader, duplicate client ID, unavailable feedback port, and commands from a non-subscribed sender.

**Assertions:** Snapshot revision, programmer state, playback state, subscribers, audit tail, and DMX receiver counts before and after each invalid packet. Only explicitly documented connection bookkeeping may change.

**Pass condition:** Invalid input returns or logs a useful failure where possible, never panics, never mutates unrelated state, and never broadcasts feedback to another client.

## API-001 — Authentication and revision conflict

**Priority:** P0  
**Primary layer:** REST integration

**Actions:** Attempt protected reads/writes without a token, with an invalid token, with the current revision, and with a stale `If-Match` revision.

**Assertions:** Authentication failures and revision conflicts have stable status codes and bodies. The stale write makes no partial mutation. The successful write increments the revision exactly once.

**Pass condition:** Clients cannot bypass authentication or overwrite newer state silently.

## API-002 — CRUD produces matching events and audit records

**Priority:** P1  
**Primary layer:** REST/WebSocket integration

**Actions:** Create, rename, and delete a group and a cue while an authenticated event socket is open.

**Assertions:** Each accepted mutation has one strictly increasing revision, one appropriate WebSocket event, and one audit entry containing the acting user/session and object identity.

**Pass condition:** REST response, event stream, audit log, and subsequent reads describe the same mutation order.

## CROSS-001 — Equivalent group value through four surfaces

**Priority:** P0  
**Primary layer:** Representative Playwright E2E

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
| API-001 | Test concurrent writers and token invalidation after session shutdown. | Compare pre/post object and revision; failed writes must be byte-for-byte unchanged. |
| API-002 | Reconnect the event socket and request audit from the last seen revision. | Compare response revision, broadcast revision, and audit revision. |
| CROSS-001 | Add fixture selection, cue GO, and group membership as further equivalence families. | Normalize surface-specific metadata before diffing application state. |
| CROSS-002 | Disconnect/reconnect WebSocket and mutate while offline. | Determine whether divergence begins in event delivery, refetch, or UI rendering. |
