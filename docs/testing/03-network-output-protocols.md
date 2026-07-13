# Network Output Protocols

These scenarios always use real loopback UDP sockets and the production packet encoders. API-returned logical frames are supporting evidence, not substitutes for received datagrams.

## How to run this file

Bind receivers before opening the show so route targets can use their actual ports. Clear receiver buffers after setup, mark them immediately before the action, and request one manual frame. Decode datagrams independently of server response objects. For negative routing cases, use a short bounded receive window and also prove another enabled route delivered, so silence cannot be explained by a failed tick.

## DMX-001 — Exact single-byte conversion

**Priority:** P0  
**Primary layer:** Rust parameterized test plus one E2E path

**Cases:** Verify 0%, 25%, 50%, 75%, and 100% produce 0, 64, 128, 191, and 255. Include values immediately around rounding boundaries.

**Assertions:** The engine's normalized value, logical DMX snapshot, Art-Net byte, and sACN byte are equal to the same expected integer for every case.

**Pass condition:** Logical values, Art-Net slots, and sACN slots use the same documented rounding behavior.

## DMX-002 — ArtDMX fields and sequence

**Priority:** P0  
**Primary layer:** Rust packet test plus Playwright E2E

**Actions:** Mark the receiver, emit three manual frames, and decode all packets after the mark.

**Assertions:**

- Header is `Art-Net\0` and opcode is ArtDMX.
- Protocol version, universe, payload length, and slot bytes are correct.
- Sequence increments once per transmitted route frame, never emits zero, and wraps from 255 to 1.
- A disabled Art-Net route emits no packet.

**Pass condition:** Received datagrams conform to Art-Net and sequencing belongs to the actual transmitted stream.

## DMX-003 — E1.31 fields, priority, and termination

**Priority:** P0  
**Primary layer:** Rust packet test plus Playwright E2E

**Assertions:** Root, framing, and DMP vectors are valid; CID and source name are stable; universe and property count are correct; start code is zero; default priority is 100; sequence increments and wraps correctly.

**Termination case:** Disable or remove an active sACN route and assert that the receiver gets the required terminated stream packets with the termination option set.

**Pass condition:** sACN receivers can identify, order, prioritize, and terminate the stream using only the wire data.

## DMX-004 — Remapped and multiple routes

**Priority:** P1  
**Primary layer:** Playwright/API E2E

**Setup:** Route logical universe 1 to Art-Net universes 10 and 11 and sACN universe 101. Add a disabled route to universe 102.

**Actions:** Program distinct values in logical universe 1 and emit one frame.

**Assertions:** Three enabled destinations receive identical slot data under their configured destination universes. Universe 102 receives nothing. Packet sequences advance independently per protocol and destination universe.

**Pass condition:** Routing changes addressing and fan-out without changing logical output or leaking to disabled routes.

## DMX-005 — Patch overlap and universe boundaries

**Priority:** P0  
**Primary layer:** API/Rust integration

**Cases:**

- Reject two fixtures occupying the same address range.
- Accept adjacent fixtures without a false overlap.
- Reject a fixture footprint extending beyond address 512.
- Patch fixtures on universes 1 and 2 and prove they reach only their matching logical routes.
- Move a fixture between universes and verify the old slot returns to its default.

**Assertions:** Invalid requests return the documented validation status, preserve the previous patch revision, and emit no partial route change. Valid boundary cases compile and produce expected bytes only in their assigned universe.

**Pass condition:** Invalid patches fail atomically and valid boundary changes cannot leave stale output behind.

## DMX-006 — 16-bit component order and defaults

**Priority:** P1  
**Primary layer:** Rust integration

**Cases:** Test MSB-first and LSB-first parameters at minimum, midpoint, maximum, and rounding boundaries. Include a multi-head fixture, virtual dimmer, inverted range, and non-zero default.

**Assertions:** Assert each component offset and byte independently, then reconstruct the 16-bit value from received bytes and compare it with the expected encoded integer.

**Pass condition:** Component mapping respects fixture metadata and produces deterministic coarse/fine bytes.

## DMX-007 — Output failure and recovery

**Priority:** P2  
**Primary layer:** Server integration

**Actions:** Cause a route send failure, inspect output health and audit state, restore the receiver, and emit another frame.

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
