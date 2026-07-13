# End-to-End Scenario Specifications

These documents expand the stable IDs in the [canonical test catalog](../help/02-typical-workflow-tests.md) into executable scenarios. They describe behavior to test; they do not imply that every scenario is implemented yet.

## Scenario documents

- [Foundational dimmers and groups](01-foundational-dimmers-and-groups.md) covers patching, ordered groups, live references, direct values, clear stages, and exact DMX.
- [Cues, tracking, and arbitration](02-cues-tracking-and-arbitration.md) covers recording, tracking, cue-only, fades, navigation, HTP, and LTP.
- [Network output protocols](03-network-output-protocols.md) covers real Art-Net and sACN packets, routing, sequence numbers, priorities, and termination.
- [OSC, API, and cross-surface agreement](04-osc-api-and-cross-surface.md) covers OSC hardware behavior, REST revisions, WebSocket/audit events, and equivalent commands.
- [Virtual time, persistence, and recovery](05-virtual-time-persistence-and-recovery.md) covers exact timing boundaries, restart behavior, corrupt data, and packaged desktop ownership.

## Common conventions

- **Bench A** is the twelve-dimmer show from the canonical catalog: fixtures 1–12 at universe 1, addresses 1–12; groups 1–3; Art-Net universe 1; and sACN universe 101.
- **Bench B** is the mixed theater show from the canonical catalog.
- Each browser scenario starts with a new active show, a fresh session/programmer, a reset virtual clock at `2020-01-01T00:00:00Z`, empty receiver buffers, and no OSC subscribers.
- A protocol assertion records the receiver mark before the action and accepts only a packet received after that mark.
- Lighting durations are virtual. A test may use wall time only for browser mechanics such as long-press recognition and process startup deadlines.
- Exact DMX conversion uses the production encoder. Representative expectations include 0% = 0, 25% = 64, 50% = 128, 75% = 191, and 100% = 255.
- Cross-surface tests should prove a representative path end to end. Exhaustive permutations belong in Rust unit or integration tests.

## Execution template

Every automated scenario should follow the same visible structure:

1. **Build the fixture.** Start from a new temporary data directory or a documented saved fixture. Create the show through `LightBench` or load a versioned fixture file. Never depend on a previous scenario.
2. **Establish observers.** Authenticate the API driver, connect the event socket, bind Art-Net/sACN receivers, and subscribe OSC hardware if the scenario needs it. Record event and packet marks before the action.
3. **Perform the action through the named surface.** UI scenarios click real controls; command-line scenarios operate the Lightning Desk keys; OSC scenarios send UDP; API scenarios use authenticated requests with explicit revisions.
4. **Synchronize on evidence.** Wait for a revision, audit/WebSocket event, programmer state, OSC return, or packet newer than the mark. Do not use a sleep as proof that an action finished.
5. **Advance application time.** Move to each stated virtual timestamp and request exactly one output frame. Record the returned virtual time and packet sequences.
6. **Make assertions.** Assert state at the narrowest layer first, then visible UI, audit/events, and real wire output where the scenario claims end-to-end behavior. Negative assertions use a bounded packet/event window.
7. **Clean up.** Unsubscribe OSC clients, close sockets and pages, stop the worker server, and remove the temporary directory. Packaged-app tests additionally prove child-process shutdown.
8. **Follow up.** If the primary scenario passes, run its listed boundary or alternate-surface cases. If it fails, retain the standard artifacts and identify the first layer where actual state diverged.

In these documents, **Assertions** are the exact checks made by the test. **Pass condition** is the product-level conclusion supported by those checks. **Follow-ups** are deliberately separate tests or failure investigations, not extra unbounded work inside the primary scenario.

## Priority

- **P0:** required to trust programming and output for a basic show.
- **P1:** required before relying on tracked playback, hardware control, or recovery.
- **P2:** important resilience, interoperability, and scale coverage.
