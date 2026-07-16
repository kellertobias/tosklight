# End-to-End Scenario Specifications

These documents expand the stable IDs in the [canonical test catalog](../help/99-Development/02-test-bench-coverage.md) into executable scenarios. They describe behavior to test; they do not imply that every scenario is implemented yet.

`00-generate-show-files.md` and `01-foundational-dimmers-and-groups.md` have Playwright coverage in the repository-root `tests/` folder. The remaining scenario documents are specifications only until matching specs are added there.

## Scenario documents

- [Reusable show files and Save As](00-generate-show-files.md) defines the two maintained canonical fixtures and first proves that Save As creates independent working copies.
- [Foundational dimmers and groups](01-foundational-dimmers-and-groups.md) covers patching, ordered groups, live references, direct values, clear stages, and exact DMX.
- [Cues, tracking, and arbitration](02-cues-tracking-and-arbitration.md) covers recording, tracking, cue-only, fades, navigation, Cuelist settings, Move in Black, HTP, and LTP.
- [Network output protocols](03-network-output-protocols.md) covers real Art-Net and sACN packets, routing, sequence numbers, priorities, and termination.
- [OSC, API, and cross-surface agreement](04-osc-api-and-cross-surface.md) covers OSC hardware behavior, REST revisions, WebSocket/audit events, and equivalent commands.
- [Virtual time, persistence, and recovery](05-virtual-time-persistence-and-recovery.md) covers exact timing boundaries, restart behavior, corrupt data, and packaged desktop ownership.
- [Preload modes and virtual playbacks](06-preload-modes-and-virtual-playbacks.md) covers the three independent Preload capture domains, all eight Settings combinations, physical and virtual playback action queues, Programmer Fade execution, and programmer-only release.
- [Playback Configuration](07-playback-configuration.md) covers Set-plus-playback modal entry, assignments, colors, clearing, type-specific layouts, Cuelist controls, Master/X-fade/Temp faders, and temporary LTP/Swap behavior.

## Common conventions

- **Compact Rig** means the maintained canonical `compact-rig.show`: twelve dimmers numbered 1–12 on layer `Dimmers`, four RGB LEDs with virtual dimmers numbered 21–24 on layer `LEDs`, and a stored empty Group 4 named `Center Spot`.
- **Default Stage Show** means the maintained canonical `default-stage.show` containing the complete built-in default rig.
- Every scenario begins with a **Starting show** line. It loads one canonical file, immediately uses Save As with a unique scenario-specific filename, and performs the test only against that active working copy.
- Never modify a canonical file in place and never reuse a working copy from another scenario. Each browser scenario also starts with a fresh session/programmer, a reset virtual clock at `2020-01-01T00:00:00Z`, empty receiver buffers, and no OSC subscribers.
- A protocol assertion records the receiver mark before the action and accepts only a packet received after that mark.
- Lighting durations are virtual. A test may use wall time only for browser mechanics such as long-press recognition and process startup deadlines.
- Exact DMX conversion uses the production encoder. Representative expectations include 0% = 0, 25% = 64, 50% = 128, 75% = 191, and 100% = 255.
- Cross-surface tests should prove a representative path end to end. Exhaustive permutations belong in Rust unit or integration tests.
- Programmer values belong to the logged-in user and are shared across that user's sessions. Desk interaction state is separate: each control desk keeps its own in-progress command line, page, and button state. OSC input subscribed to a desk alias joins that desk's UI interaction state; completing a value command then writes into the user's shared programmer.

### Literal operator-action notation

The procedures in these files are intentionally literal. A tester must not fill in an omitted selection, recording, or confirmation step from experience with another console.

- **Click** means one ordinary primary-button click or one finger tap. Do not hold Command, Control, Shift, or another modifier unless the procedure explicitly says so.
- Consecutive fixture and group clicks are additive while the selection is still current. For example, click fixture 5 and then fixture 6 to obtain the ordered selection `5, 6`. A value change, encoder move, or preset recall applies to the current selection without immediately deselecting it. The next fixture or group click starts a new selection while previously programmed values remain active; a leading `[+]` continues the current selection instead.
- `[KEY]` means press the named Lightning Desk keypad key once. Text such as `5 [+] 6 [ENTER]` is the exact key order, not a summary of the resulting command line.
- For a Group term, the first `[GRP]` press displays `GROUP`. A second consecutive `[GRP]` press replaces `GROUP` with `DEGRP`; it does not append a second word. `DEGRP <number>` dereferences only that Group term into its current individual fixtures. Merely using Group as the persistent default mode never dereferences a Group.
- **Press `[REC]`, then click target** means arm Record first and then click the named pool cell. When a populated existing Group is the target, the recording dialog presents three explicit actions: **Merge**, **Overwrite**, and **Cancel**. Merge and Overwrite perform the named operation; Cancel closes the dialog, disarms Record, and makes no change. Empty pool cells and stored empty Groups record directly without asking for Merge or Overwrite.
- **Merge Group** retains the existing ordered members and appends only selected fixtures that are not already members. **Overwrite Group** replaces the complete ordered membership with the current resolved selection.
- In a fully entered command, `[REC]` without a modifier overwrites, `[REC] [+]` merges, and `[REC] [-]` subtracts. Group operations use the current selection; Cue operations use the fixture/group attribute addresses currently active in the programmer. Record-minus with an empty applicable source deletes the explicit Group or Cue target. It must have the same persisted result as the corresponding `[DEL]` command.
- `[-]` subtracts the fixture or range on its right from the ordered selection on its left. Retained fixtures keep their relative order. If a subtracted fixture is added again later with `[+]`, it is a new addition at the end of the selection rather than returning to its former position. For example, `[GRP] [3] [-] [2] [+] [2] [ENTER]` resolves Group 3 without fixture 2 and then appends fixture 2 at the end.
- To persist a subtracted or reordered Group selection without a dedicated Group editor, press `[REC]`, click the Group target, and choose **Overwrite**, or enter `[REC] [GRP] <number> [ENTER]`, to store the resolved order. To remove the current selection directly from an existing Group, enter `[REC] [-] [GRP] <number> [ENTER]`. A full manual rebuild remains available when the requested reorder cannot be expressed through ordered subtraction and addition.
- A procedure labelled **Harness only** has no operator control. Execute the listed REST, WebSocket, OSC, virtual-clock, process, or file-fixture operation in the test driver. Do not replace it with an unrelated UI gesture.
- A procedure labelled **UI capability required** describes an intended operator workflow for which the named control is not currently available. The `@ui` case is expected to remain unimplemented or fail at that exact step until the capability exists; the API/Rust variant may still be runnable.

## Mandatory API and UI pairing

Every catalog scenario ID is implemented as two independent tests:

- `<ID> @api` performs the behavior directly through authenticated REST or the versioned command WebSocket. It does not open a browser. This is the server, engine, persistence, and protocol contract.
- `<ID> @ui` starts from a separate fresh working copy, performs the equivalent behavior through the production UI, and runs the same assertion function as the API test.

Both variants use the same canonical starting show, arrangement function, virtual timestamps, normalized expected state, and output oracle. They must not run sequentially against the same show: independent fixtures are what make the comparison trustworthy.

| API result | UI result | Meaning |
| --- | --- | --- |
| Pass | Pass | Server contract and UI adapter agree. |
| Fail | Fail | Investigate the API/server failure first; the UI result is downstream noise until the contract passes. |
| Pass | Fail | The server contract is intact; investigate UI selectors, interaction semantics, WebSocket synchronization, or rendering. |
| Fail | Pass | Treat as an invalid pairing or an API-only regression; the variants may not be exercising the same operation. |

Use `pairedScenario(...)` from `apps/control-ui/e2e/bench/pairedScenario.ts` to register both variants. New catalog scenarios must not be added as a lone `test(...)`. OSC, Art-Net packet-layout, packaged-desktop, CSS/layout, and test-bench self-tests may add `@osc`, `@wire`, `@desktop`, or `@bench` coverage, but these are supplemental checks rather than substitutes for the API/UI pair when the behavior is operator-visible.

The runner exposes separate commands so CI classifies failures clearly:

- `./test e2e-api`
- `./test e2e-ui`
- `./test e2e-supplemental`
- `./test e2e` for the combined local run

CI shards the API and UI catalogs independently. This keeps both sides of every pair fast as the catalog grows while preserving distinct failure jobs and artifacts.

## Execution template

Every automated scenario should follow the same visible structure:

1. **Create the working copy.** Follow the scenario's **Starting show** line: load the named canonical show, immediately use Save As with the scenario-specific filename, confirm the copy is active, and mutate only that copy.
2. **Establish observers.** Authenticate the API driver, connect the event socket, bind Art-Net/sACN receivers, and subscribe OSC hardware if the scenario needs it. Record event and packet marks before the action.
3. **Perform the paired actions.** In the `@api` variant, use authenticated REST or the command WebSocket. In the `@ui` variant, click real controls or operate the Lightning Desk keys. Optional OSC or desktop variants use their real transports.
4. **Synchronize on evidence.** Wait for a revision, audit/WebSocket event, programmer state, OSC return, or packet newer than the mark. Do not use a sleep as proof that an action finished.
5. **Advance application time.** Move to each stated virtual timestamp and request exactly one output frame. Record the returned virtual time and packet sequences.
6. **Run the shared assertions.** Both variants call the same normalized state and wire-output oracle. The UI variant may add visible-state assertions, but it may not weaken or replace the shared contract. Negative assertions use a bounded packet/event window.
7. **Clean up.** Unsubscribe OSC clients, close sockets and pages, stop the worker server, and remove the temporary directory. Packaged-app tests additionally prove child-process shutdown.

## Visual inspection recording

Run `./test record` to produce one narrated 1920×1080 WebM walkthrough at `artifacts/visual-inspection/light-visual-inspection.webm`. This is intentionally separate from the fast catalog. The recording keeps the complete desk application visible and adds external observer panels for:

- OSC packets sent by the simulated physical controller and decoded feedback returned by the subscribed desk alias;
- the desk-local command line after each UI or OSC button press;
- logical DMX values from `/api/v1/dmx`; and
- the actual UDP values received from the configured Art-Net and sACN outputs.

The recording is supplementary evidence. The normal Playwright assertions remain authoritative because video timing and encoding are not used as synchronization or pass criteria.
8. **Follow up.** If the primary scenario passes, run its listed boundary or alternate-surface cases. If it fails, retain the standard artifacts and identify the first layer where actual state diverged.

In these documents, **Assertions** are the exact checks made by the test. **Pass condition** is the product-level conclusion supported by those checks. **Follow-ups** are deliberately separate tests or failure investigations, not extra unbounded work inside the primary scenario.

## Priority

- **P0:** required to trust programming and output for a basic show.
- **P1:** required before relying on tracked playback, hardware control, or recovery.
- **P2:** important resilience, interoperability, and scale coverage.
