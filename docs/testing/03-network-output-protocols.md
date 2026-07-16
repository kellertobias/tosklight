# Network Output Protocols

These scenarios always use real loopback UDP sockets and the production packet encoders. API-returned logical frames are supporting evidence, not substitutes for received datagrams.

## How to run this file

Bind receivers before opening the show so route targets can use their actual ports. Start every scenario by loading its named canonical show and immediately using Save As to create the stated working copy. Clear receiver buffers after setup, mark them immediately before the action, and request one manual frame. Decode datagrams independently of server response objects. For negative routing cases, use a short bounded receive window and also prove another enabled route delivered, so silence cannot be explained by a failed tick.

## DMX-001 — Exact single-byte conversion

**Priority:** P0  
**Primary layer:** Rust parameterized test plus one E2E path

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `dmx-001.show`, and use dimmer fixture 1 at universe 1, address 1 in the active copy.

**Detailed procedure:**

1. Open Stage or Fixture Sheet and click fixture 1 once.
2. For each value `0`, `25`, `50`, `75`, and `100`, touch Intensity, enter the value, and confirm it. Wait for the programmer revision before continuing.
3. Immediately before each output action, reset/mark both UDP receivers. Advance virtual time to the configured programmer fade endpoint, or use a zero-duration programmer fade for this conversion-only case, and request exactly one frame.
4. Decode universe 1 address 1 from the returned logical frame, Art-Net packet, and sACN packet. Expect `0`, `64`, `128`, `191`, and `255` respectively.
5. **Harness only:** submit normalized values immediately below, exactly at, and immediately above each half-byte rounding threshold through `programmer.set`; the UI percentage field is not precise enough for these boundary values.

**Assertions:** The engine's normalized value, logical DMX snapshot, Art-Net byte, and sACN byte are equal to the same expected integer for every case.

**Pass condition:** Logical values, Art-Net slots, and sACN slots use the same documented rounding behavior.

## DMX-002 — ArtDMX fields and sequence

**Priority:** P0  
**Primary layer:** Rust packet test plus Playwright E2E

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `dmx-002.show`, and use the active copy for this scenario.

**Detailed procedure:**

1. Bind the Art-Net receiver and update the working copy's enabled Art-Net route destination to that receiver through a revision-checked route-object PUT.
2. Record the receiver mark and current route sequence state.
3. Call `POST /api/v1/test/clock/advance` with `{"millis":0}` three separate times. Do not combine them into one 0 ms call and do not wait for the scheduler.
4. Collect exactly the ArtDMX packets received after the mark, preserve their raw bytes, and decode each header and payload independently.
5. **Harness only:** seed the route sequence at 254, emit three more frames, and verify `255, 1, 2`. Disable the route with a revision-checked PUT, mark the receiver, emit once, and verify no packet inside the bounded receive window while the enabled sACN route still receives one.

**Assertions:**

- Header is `Art-Net\0` and opcode is ArtDMX.
- Protocol version, universe, payload length, and slot bytes are correct.
- Sequence increments once per transmitted route frame, never emits zero, and wraps from 255 to 1.
- A disabled Art-Net route emits no packet.

**Pass condition:** Received datagrams conform to Art-Net and sequencing belongs to the actual transmitted stream.

## DMX-003 — E1.31 fields, priority, and termination

**Priority:** P0  
**Primary layer:** Rust packet test plus Playwright E2E

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `dmx-003.show`, and use the active copy for this scenario.

**Detailed procedure:**

1. Bind the sACN receiver and point the working copy's enabled sACN route at it with a revision-checked route-object PUT.
2. Mark the receiver and call the 0 ms virtual-clock advance three times. Save and decode all E1.31 datagrams received after the mark.
3. Check the stable CID/source name, universe 101, priority 100, DMP property count, start code, sequence, and slot payload on each packet.
4. **Harness only:** seed the route sequence at 254 and emit three frames to prove the production sequence helper emits `255, 1, 2`; it deliberately skips zero after wrapping.
5. Mark the receiver, then disable or delete the active sACN route through its current revision. Collect the complete termination burst and assert the stream-terminated option on every required packet.

**Assertions:** Root, framing, and DMP vectors are valid; CID and source name are stable; universe and property count are correct; start code is zero; default priority is 100; sequence increments and wraps correctly.

**Termination case:** Disable or remove an active sACN route and assert that the receiver gets the required terminated stream packets with the termination option set.

**Pass condition:** sACN receivers can identify, order, prioritize, and terminate the stream using only the wire data.

## DMX-004 — Remapped and multiple routes

**Priority:** P1  
**Primary layer:** Playwright/API E2E

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `dmx-004.show`, and use the active copy for this scenario.

**Detailed procedure:**

1. Bind four UDP receiver destinations. Through authenticated revision-checked route-object PUTs, create enabled logical-1 routes to Art-Net universe 10, Art-Net universe 11, and sACN universe 101, plus a disabled sACN route to universe 102.
2. **Harness only:** the current DMX **Universe routes** view is read-only; there is no route editor to perform this setup by touch.
3. Click fixture 1, set Intensity to 25%, click fixture 2, set it to 50%, click fixture 3, and set it to 75%.
4. Advance to the programmer fade endpoint. Mark all four receivers, then advance 0 ms to emit one comparison frame.
5. Decode the newest packet at each enabled destination and run a bounded receive check against disabled universe 102.

**Assertions:** Three enabled destinations receive identical slot data under their configured destination universes. Universe 102 receives nothing. Packet sequences advance independently per protocol and destination universe.

**Pass condition:** Routing changes addressing and fan-out without changing logical output or leaking to disabled routes.

## DMX-005 — Patch overlap and universe boundaries

**Priority:** P0  
**Primary layer:** API/Rust integration

**Starting show:** For each case, load canonical `compact-rig.show`, immediately Save As `dmx-005-<case>.show`, remove all patched fixtures from the active copy, and then patch only the fixtures named below. Discard that working copy before the next case.

**Detailed cases:**

1. For each case, use **Setup → Patch** only to inspect the result; use authenticated object PUTs with explicit expected revisions for deterministic patch creation and validation.
2. Put dimmer fixture 1 at `1.1`, record the patch revision, then PUT dimmer fixture 2 at `1.1`. Expect rejection, reread the patch, and prove fixture 1 and the revision are unchanged.
3. In a fresh copy, PUT fixture 1 at `1.1` and fixture 2 at `1.2`; expect both writes to succeed and inspect both rows in Patch.
4. In a fresh copy, PUT RGB fixture 21 at `1.511`; expect rejection because its three-channel footprint would end at 513. Reread the patch and prove no partial fixture exists.
5. In a fresh copy, PUT dimmer 1 at `1.1` and dimmer 2 at `2.1`; create one enabled route for each logical universe, program different values, mark receivers, and emit one frame. Each destination must contain only its matching universe.
6. In a fresh copy, PUT dimmer 1 at `1.1`, program it, and emit a frame. PUT the same patched-fixture object at `2.1` using its current revision, emit again, and prove U1.1 returned to default while U2.1 carries the value.
7. **UI capability note:** Patch edits can be armed with `[SET]`, but destructive bulk removal and every invalid-address case do not have a sufficiently deterministic touch workflow for this atomicity test. Do not silently mix UI state from one case into another.

**Assertions:** Invalid requests return the documented validation status, preserve the previous patch revision, and emit no partial route change. Valid boundary cases compile and produce expected bytes only in their assigned universe.

**Pass condition:** Invalid patches fail atomically and valid boundary changes cannot leave stale output behind.

## DMX-006 — 16-bit component order and defaults

**Priority:** P1  
**Primary layer:** Rust integration

**Starting show:** Load canonical `default-stage.show`, immediately Save As `dmx-006.show`, and use the active copy. Use its multi-head RGB Sunstrip fixture 501 for the multi-head and virtual-dimmer cases; create isolated fixture-definition variants for MSB/LSB, inversion, and non-zero-default cases before patching each variant into an unused universe.

**Detailed harness procedure:**

1. Clone the required fixture definitions into isolated test definitions: one MSB-first 16-bit parameter, one LSB-first parameter, one inverted range, and one non-zero default. Give every variant a new definition ID/revision.
2. Patch one variant at a time into an unused logical universe with a dedicated receiver route; record its exact coarse and fine offsets before programming.
3. Set minimum, midpoint, maximum, and values around each 16-bit rounding boundary through `programmer.set`. Emit one 0 ms frame per value and assert the two bytes independently before reconstructing the integer.
4. Repeat against logical heads 501.1–501.10 and fixture 501's virtual dimmer/color emitters, addressing the specific head rather than the master when required.
5. Release the programmed value and emit another frame to verify the configured non-zero default and inverted mapping.
6. **Harness only:** the Fixture Library editor does not expose byte-order/default mutation as a concise operator workflow suitable for this parameterized test.

**Assertions:** Assert each component offset and byte independently, then reconstruct the 16-bit value from received bytes and compare it with the expected encoded integer.

**Pass condition:** Component mapping respects fixture metadata and produces deterministic coarse/fine bytes.

## DMX-007 — Output failure and recovery

**Priority:** P2  
**Primary layer:** Server integration

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `dmx-007.show`, and use the active copy for this scenario.

**Detailed harness procedure:**

1. Configure two enabled routes: one to a healthy bound receiver and one to an injectable destination whose send operation can be forced to fail.
2. Record diagnostics, audit revision, packet marks, and route error counters. Enable the injected send failure for only the second destination.
3. Emit one 0 ms frame. Verify the healthy receiver got the frame, then reread diagnostics/audit and prove only the failing route's error counter changed.
4. Disable the injection without recreating the route. Mark both receivers and emit another 0 ms frame.
5. Verify both destinations receive current state with valid next sequences rather than a replay of the failed frame.
6. **Harness capability required:** if the output layer has no route-scoped send-failure injection seam, this scenario is blocked there; closing a UDP receiver is not a valid substitute because UDP send normally still succeeds.

**Assertions:** Send errors increment without stopping other routes. Recovery resumes packets with valid sequences and current state rather than replaying stale frames.

**Pass condition:** One failing destination is observable and isolated from healthy output routes.

## Follow-ups

| Scenario | Next tests after the primary case | First failure checks |
| --- | --- | --- |
| DMX-001 | Property-test all byte rounding thresholds and fixture curves. | Compare normalized value, encoded integer, and received slot separately. |
| DMX-002 | Test sequence wrap, odd payload lengths, and maximum 512 slots. | Save the raw datagram and decode header offsets before blaming routing. |
| DMX-003 | Test custom priority, source identity stability across restart, and all termination packets. | Inspect raw root/framing/DMP lengths and option bits. |
| DMX-004 | Add routes dynamically while output is active and restart with saved routes. | Compare logical universe, route mapping, destination address, and per-route sequence key. |
| DMX-005 | Test multi-head footprints, multipatch, and remapping an active fixture. | Verify patch validation and compiled slot ownership before engine output. |
| DMX-006 | Add physical curves, inversion, virtual dimmers, and head-shared parameters. | Compare fixture metadata, normalized component value, and coarse/fine byte order. |
| DMX-007 | Fail only one of several routes and repeat during shutdown. | Inspect health counters and healthy-route packets at the same virtual frame. |
