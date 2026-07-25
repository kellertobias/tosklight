# End-to-End Testing Conventions

The [canonical test catalog](../help/99-Development/02-test-bench-coverage.md) indexes stable scenario IDs. Implemented scenarios live with their executable Playwright coverage in the repository-root `tests/` folder. Harness-only, packaged-process, and inherently surface-specific cases state their actual execution shape instead of claiming an artificial API/UI pair.

## Retained scenario contract

- [OSC, API, and cross-surface agreement](04-osc-api-and-cross-surface.md) covers OSC hardware behavior, REST revisions, WebSocket/audit events, equivalent commands, authoritative PREV/NEXT/ALL and independent HIGH boundaries, and the focused Matter bridge transport/UI boundaries.
- [Feature 21 completion coverage](../plans/Done/21-completion-coverage-and-release-verification.DONE.md) retains the extended `SHOW-005`, `UPDATE-002`, `HIGHLIGHT-004` through `HIGHLIGHT-006`, `FIXTURE-002`, and `MATTER-002` release-verification contracts.
- [Client history and removal](../plans/Done/22-client-history-and-removal.DONE.md) retains the `CLIENT-001` desk-persistent presence, history, removal, and clean re-registration contract.
- [Planned demo run](../plans/Done/23-planned-demo-run.DONE.md) retains the single `DEMO-001` narrated show-build, programming, busking, and Preload contract.
- [Return Home in the Position special dialog](../plans/Done/25-return-home-position-special-dialog.DONE.md) retains the paired `POSITION-HOME-001` per-head defaults, atomic programmer gesture, Undo, and software/hardware layout contract.
- [Color special-dialog alignment](../plans/Done/26-color-special-dialog-alignment.DONE.md) retains the paired `COLOR-RANGE-001` uniform click, ordered range, cancellation, Undo, and software/hardware Shift contract.
- [Hardware-connected playback selection](../plans/Done/28-hardware-connected-playback-selection.DONE.md) retains the paired `PLAYBACK-SELECT-001` card ownership, whole-area concrete Record target, Group distinction, explicit-page identity, real-control isolation outside Record, and OSC convergence contract.
- [Command Line history](../plans/Done/30-command-line-history-panel.DONE.md) retains the paired `COMMAND-HISTORY-001` accepted/rejected ordering, non-executing reuse, input preservation, transient bounded retention, redaction, reconnect, hardware layout, and OSC attribution contract.
- [Hardware-connected encoder display](../plans/Done/31-hardware-connected-encoders.DONE.md) retains the `ENCODER-DISPLAY-001` six-slot numbering, value formatting, remapping, physical turn/press-turn, stable family mappings, and measured hardware-layout contract.
- [Record and Update workflow colors](../plans/Done/33-record-and-update-menu-colors.DONE.md) retains the `WORKFLOW-COLOR-001` semantic red/amber tokens, text identity, distinct destructive/error/disabled actions, and software/hardware visual contract.
- [Active playback colors](../plans/Done/34-active-playback-colors.DONE.md) retains the `PLAYBACK-COLOR-001` configured-color runtime strength, separate selection outline, combined-state, empty-cell, hardware, and Virtual Playback contract.
- [Fixture Address screen](../plans/Done/35-fixture-address-screen.DONE.md) retains the `FIXTURE-ADDRESS-001` complete-footprint availability, own-slot exclusion, integrated number block, split-wide atomic validation, cancellation, and supported-layout contract.
- [Cuelist and Cue Settings layout](../plans/Done/36-cuelist-and-cue-settings-layout.DONE.md) retains the `CUELIST-LAYOUT-001` compact thumbnail-led Cue editor, frameless rows, non-scrolling SET/value fallback, modal three-column Cuelist Settings, title-bar actions, visible selection-only table, dirty-close decision, and narrow-layout contract.
- [Chaser crossfade percentage](../plans/Done/37-chaser-crossfade-percentage.DONE.md) extends `CUE-012` with the persisted `0–100%` setting, exact snap/half/full-step transitions, live BPM/multiplier resolution, deterministic legacy migration, and timing-bypass contract.

This file remains because OSC-002, OSC-004, OSC-006, API-002, and CROSS-001 still contain contract assertions that are not fully represented by executable tests.

## Common conventions

- **Compact Rig** means the maintained canonical `compact-rig.show`: twelve dimmers numbered 1–12 on layer `Dimmers`, four RGB LEDs with virtual dimmers numbered 21–24 on layer `LEDs`, and a stored empty Group 4 named `Center Spot`.
- **Default Stage Show** means the maintained canonical `default-stage.show` containing the complete built-in default rig.
- Every show-mutating scenario begins with a **Starting show** line. It loads one canonical file, immediately uses Save As with a unique scenario-specific filename, and performs the test only against that active working copy. File-system, desk-lock, packaged-process, and layout-only scenarios instead state their own isolated bench setup and do not create a meaningless show copy.
- Never modify a canonical file in place and never reuse a working copy from another scenario. Each browser scenario also starts with a fresh session/programmer, a reset virtual clock at `2020-01-01T00:00:00Z`, empty receiver buffers, and no OSC subscribers.
- A protocol assertion records the receiver mark before the action and accepts only a packet received after that mark.
- Lighting durations are virtual. A test may use wall time only for browser mechanics such as long-press recognition and process startup deadlines.
- Exact DMX conversion uses the production encoder. Representative expectations include 0% = 0, 25% = 64, 50% = 128, 75% = 191, and 100% = 255.
- Cross-surface tests should prove a representative path end to end. Exhaustive permutations belong in Rust unit or integration tests.
- Programmer values belong to the logged-in user and are shared across that user's sessions. Desk interaction state is separate: each control desk keeps its own in-progress command line, open ordered selection gesture and source references, page, and button state. OSC input subscribed to a desk alias joins that desk's UI interaction state. Another desk used by the same user may build a different partial selection at the same time. Completing a value command closes only the originating desk's open gesture and writes the resulting fixture- or Group-scoped value into the user's shared programmer, where every session for that user sees it.

### Literal operator-action notation

The procedures in the retained scenario contract and executable tests are intentionally literal. A tester must not fill in an omitted selection, recording, or confirmation step from experience with another console.

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
- A procedure labelled **Harness boundary** identifies setup or inspection that has no truthful operator gesture. The executable supplemental case uses the named REST, process, file, or deterministic runtime seam and is expected to pass; an open browser must not conceal the same harness mutation and be called an independent UI action.

## API and UI pairing

Every catalog behavior that can be performed independently through both the authenticated API and production UI is implemented as two independent tests:

- `<ID> @api` performs the behavior directly through authenticated REST or the versioned command WebSocket. It does not open a browser. This is the server, engine, persistence, and protocol contract.
- `<ID> @ui` starts from a separate fresh working copy, performs the equivalent behavior through the production UI, and runs the same assertion function as the API test.

Both variants use the same canonical starting show, arrangement function, virtual timestamps, normalized expected state, and output oracle. They must not run sequentially against the same show: independent fixtures are what make the comparison trustworthy.

An ID whose complete primary action is a harness boundary or packaged-process ownership remains a supplemental `@wire`, `@restart`, `@desktop`, or Rust integration case until a genuine operator adapter exists. An inherently UI-only contract such as pane geometry, rendered Markdown, or visible terminology uses a real `@ui` case; an integrated boundary such as Desk Lock may deliberately prove UI, API rejection, and OSC suppression in one cross-surface case. These exceptions are stated in the catalog, retained contract, or executable test and are not substitutes for pairing when equivalent independent API and UI actions do exist. Do not manufacture a browser test that performs the same hidden API or file mutation behind an open page: that is not an independent UI variant. Any operator-visible setup or recovery action surrounding a harness boundary still receives an API/UI pair, as `SHOW-003` does for choosing the valid recovery show.

| API result | UI result | Meaning |
| --- | --- | --- |
| Pass | Pass | Server contract and UI adapter agree. |
| Fail | Fail | Investigate the API/server failure first; the UI result is downstream noise until the contract passes. |
| Pass | Fail | The server contract is intact; investigate UI selectors, interaction semantics, WebSocket synchronization, or rendering. |
| Fail | Pass | Treat as an invalid pairing or an API-only regression; the variants may not be exercising the same operation. |

Use `pairedScenario(...)` from `apps/control-ui/e2e/bench/pairedScenario.ts` to register both variants. New cross-surface catalog scenarios must not be added as a lone `test(...)`. OSC, Art-Net packet-layout, packaged-desktop, CSS/layout, and test-bench self-tests may add `@osc`, `@wire`, `@desktop`, or `@bench` coverage, but these are supplemental checks rather than substitutes for the API/UI pair when the behavior exists on both surfaces.

Browser-only operator contracts use `scenario(...)` from
`apps/control-ui/e2e/bench/scenario.ts`. Its callback receives typed `app`, `builtIn`,
`desktop`, `screen`, and `screenshot` intents: scenario authors name operator surfaces,
Desktop pane types, stable kebab-case pane slugs, and 24 × 18 grid rectangles without
querying selectors, pixel coordinates, runtime pane IDs, or browser globals. Pane handles
remain actionable after a Desktop is reopened, and their configuration type follows the
chosen `PaneType`. `npm run test:bench-types` protects those compile-time distinctions.
Secondary-screen helpers prove the persisted browser intent and controllable desktop-bridge
request; they do not claim that Playwright opened or captured a native OS window.

Browser scenarios establish show fixtures through the typed catalog exposed by
`show.use(Show.Empty)`, `show.use(Show.TwelveDimmers)`, `show.use(Show.CompactRig)`, or
`show.use(Show.DefaultStage)`. Each call copies an immutable canonical input into a uniquely
identified working show inside that test's temporary data directory; `show.resetWorkingCopy()`
restores only that scenario's copy. Reusable `defineShow(...)` recipes declare fixture numbers,
profile names, and Group prerequisites so stale fixtures fail during labelled setup, before the
first operator action. Filesystem paths, show IDs, and revision plumbing remain internal to the
bench. Desktops are desk data and must be established with `desktop.use(...)`, not declared as a
show prerequisite. This fast fixture setup is distinct from testing the operator-facing load
workflow, which will use the `show.load(...)` intent introduced by the next bench step.

Operator-facing show workflows now use `show.create(name)`, `show.load(show)`, `show.save()`,
`show.saveAs(name)`, `show.saveRevision(name)`, `show.loadRevision(show, revision)`,
`show.loadCleanDefault()`, and `show.restart(RestartMode.Graceful | RestartMode.Abrupt)`.
Unqualified actions use the visible browser workflow; independently meaningful production API
routes are available under `show.via.api`, while `show.via.ui` makes the browser choice explicit.
The helpers return opaque named show handles, so scenarios can load and assert identities without
runtime show IDs or paths. `show.save()` truthfully confirms continuous autosave (or keeps a
revision copy separate); ToskLight has no ordinary manual-Save mutation. Accordingly,
`show.expect.dirty(false)` proves persisted convergence and `dirty(true)` fails as unsupported
instead of inventing a dirty projection. Recovery fixture placement stays inside the isolated
bench, while `show.expect.recoveryRequired()` and `show.expect.recovered()` prove the real
readiness state, visible recovery actions, and preservation of the damaged file.

Command-line scenarios use `command.type(...)`, `command.execute(...)`,
`command.clear()`, and `command.expect(...)`; the visible route presses logical desk keys in
order and executes by clicking the visible `ENT` control. Use `keypad.press([...])` when exact
key order is itself the contract. Semantic API selection uses `fixture(number, head?)`,
`fixtureRange(...)`, `group(...)`, `groupRange(...)`, and `dereferencedGroup(...)` through
`selection.targets/add/remove/range/clear`, with `expect.selection(...)` as the normalized
ordered oracle. Selection stepping remains `selection.previous/next/all`, while independent
Highlight power is `highlight.on/off/toggle` with explicit `via.ui/api/osc` ports. Call
`hardware.connect(alias)` before an OSC port and `hardware.disconnect()` during cleanup.
Selection routes now include explicit Fixture Sheet, Stage, Group pool, real touch, keypad, API,
and subscribed OSC item adapters. Unqualified selection chooses reproducibly between eligible
API and keypad adapters and records its seed, action index, candidates, and selected route for
replay. Stage click/Shift-click requires an anchor established through the Stage route. Numeric
Stage Shift-click ranges stay unavailable because Stage order is visual rather than numeric;
`shiftClick.item(...)` instead returns the observed visible-order selection and its truthful
gesture expression. OSC item, ordered-items, and range helpers require a pristine command line
and synchronize every press against the authoritative command-line revision rather than periodic
OSC feedback.

Normalized Programmer encoders are available through the enum-backed
`encoder.<group>.<attribute>` tree. `.set(value)` is the explicit absolute path,
`.set([value, ProgrammerToken.Thru, ...])` enters a validated spread, and `.add(steps)` /
`.subtract(steps)` apply relative one-percent detents through the typed API intent. Explicit
`.via.api` and `.via.ui` routes constrain the surface; the visible route resolves the live family
and software encoder without exposing labels or physical slots to scenario bodies. Unqualified
actions record their seed, action index, candidates, and selected route.
When simulated hardware is connected, normalized encoders also expose relative
`.via.osc.add(steps)` and `.via.osc.subtract(steps)` detents. The adapter activates the visible
family, resolves the logical attribute from the live attached-hardware display instead of assuming
a physical slot, sends one detent at a time, and waits for each authoritative Programmer revision.
The OSC port is relative-only at the type level. Profile-derived discrete and special-dialog
controls plus undecided encoder press/page semantics remain queued in refactoring chunks 06d–06e.

Programmer Fade is available through `timing.programmerFade`. `set("4s")` records a seeded choice
between eligible routes; `.via.api`, `.via.valueEntry`, `.via.fader`, and connected `.via.osc`
constrain it explicitly. The value-entry adapter presses the production **Set value** action and
uses its visible number pad. The fader adapter switches to the Playback controls and performs a
real pointer slide, calibrating against authoritative 0.1-second feedback without filling the
underlying range input. Every route waits for the shared configuration value before returning.
`double()`, `half()`, and `off()` apply the corresponding time-master semantics, while
`currentMillis()` exposes the normalized assertion value.

Profile-derived discrete Programmer values are available through
`encoder.discrete.choices(attribute)`, `.set(attribute, semanticId)`, `.release(attribute)`,
`.releaseVisible(attribute)`, and `.clear()`. Choices come from fixed and indexed functions on the
selected fixtures' embedded profile revisions, retain stable semantic IDs, and target only
compatible selected fixtures. They are not flattened into a global fixture-independent enum.
Normalized encoder ports also expose `release()`, while `encoder.clear()` clears the Programmer
through the shared values authority.

Existing special controls are grouped under `special`: Position Return Home and alignment, Beam
and Shapers attributes discovered from the selected patch, and compatible profile-derived Control
actions. Visible helpers press the production family tabs, dialogs, buttons, and pointer faders.
API alignment and Control helpers use the same command boundaries as the production client; Control
actions serialize compatible fixtures across the active-show transition barrier.

The runner exposes separate commands so CI classifies failures clearly:

- `./test e2e-api`
- `./test e2e-ui`
- `./test e2e-supplemental`
- `./test e2e` for the combined local run

CI shards the API and UI catalogs independently. This keeps both sides of every pair fast as the catalog grows while preserving distinct failure jobs and artifacts.

## Execution template

Every automated scenario should follow the same visible structure:

1. **Create the isolated fixture.** For a show-mutating scenario, follow its executable setup: load the named canonical show, immediately use Save As with the scenario-specific filename, confirm the copy is active, and mutate only that copy. Otherwise create the scenario's named temporary files, roots, sessions, or process fixture.
2. **Establish observers.** Authenticate the API driver, connect the event socket, bind Art-Net/sACN receivers, and subscribe OSC hardware if the scenario needs it. Record event and packet marks before the action.
3. **Perform the named actions.** For a paired case, the `@api` variant uses authenticated REST or the command WebSocket and the `@ui` variant clicks real controls or operates the Lightning Desk keys. Surface-specific, OSC, or desktop cases use only the real surface named by the scenario.
4. **Synchronize on evidence.** Wait for a revision, audit/WebSocket event, programmer state, OSC return, or packet newer than the mark. Do not use a sleep as proof that an action finished.
5. **Advance application time.** Move to each stated virtual timestamp and request exactly one output frame. Record the returned virtual time and packet sequences.
6. **Run the shared assertions.** Both variants call the same normalized state and wire-output oracle. The UI variant may add visible-state assertions, but it may not weaken or replace the shared contract. Negative assertions use a bounded packet/event window.
7. **Clean up.** Unsubscribe OSC clients, close sockets and pages, stop the worker server, and remove the temporary directory. Packaged-app tests additionally prove child-process shutdown.
8. **Follow up.** If the primary scenario passes, run its listed boundary or alternate-surface cases. If it fails, retain the standard artifacts and identify the first layer where actual state diverged.

## Visual inspection recording

Run `./test record` to execute the complete Playwright catalog in serial recording mode. Every browser test receives its own 1920×1080 video under `.artifacts/test/results/`, with deliberately slowed browser actions and a non-interactive narration bar showing the scenario ID, current phase, purpose, recent desk events, explicit external OSC transmissions and feedback, and current logical DMX output. After the run, ffmpeg joins those clips into `.artifacts/test/visual-inspection/light-ui-test-catalog.webm`, so the entire browser-tested catalog can be watched as one reel rather than inspecting only one test.

Run `./test demo` for the single maintained `DEMO-001` product walkthrough. It opens `?demo=product`, keeps the complete application, Stage, live DMX grid, simulated keypad, and playback surface visible, and overlays compact phase titles without resizing the desk. A successful run atomically refreshes the completed portable show at `assets/demo.show` and writes the maintained video and screenshot below `.artifacts/test/visual-inspection/product-demo/`.

The catalog also includes the more detailed narrated walkthrough at `.artifacts/test/visual-inspection/light-visual-inspection.webm`. That chapter keeps the complete desk application visible and adds expanded external observer panels for:

- OSC packets sent by the simulated physical controller and decoded feedback returned by the subscribed desk alias;
- the desk-local command line after each UI or OSC button press;
- logical DMX values from `/api/v2/output/dmx`; and
- the actual UDP values received from the configured Art-Net and sACN outputs.

The recording run is intentionally slower than the normal suite: browser actions default to a 250 ms delay and narrated checkpoints remain visible for 1,200 ms. Override those defaults with `LIGHT_VISUAL_SLOW_MO=<milliseconds>` and `LIGHT_VISUAL_STEP_PAUSE=<milliseconds>` when a still slower inspection copy is useful. API-only cases have no browser surface and therefore add assertions but no video clip; their paired UI cases show the corresponding application workflow. Recordings are supplementary evidence. The normal Playwright assertions remain authoritative because video timing and encoding are not used as synchronization or pass criteria.

In the retained contract, **Assertions** are the exact checks made by the test. **Pass condition** is the product-level conclusion supported by those checks. **Follow-ups** are deliberately separate tests or failure investigations, not extra unbounded work inside the primary scenario.

## Priority

- **P0:** required to trust programming and output for a basic show.
- **P1:** required before relying on tracked playback, hardware control, or recovery.
- **P2:** important resilience, interoperability, and scale coverage.
