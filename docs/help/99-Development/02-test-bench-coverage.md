# Test Bench Coverage Catalog

This is the canonical catalog for executable acceptance coverage. Every stable ID listed here is registered in at least one repository-root Playwright test. Stable IDs must be retained when scenarios move between Rust integration tests and the Playwright browser bench. Every executable scenario records its actual action surface and oracle so a passing API-only, UI-only, integrated, or wire-protocol case is not mistaken for a different kind of coverage. Packaged desktop launchability is a CI build invariant rather than a Playwright scenario.

[Testing conventions](../../engineering/test-map.md) are maintained in the engineering test map. Implemented scenario procedures live with their repository-root Playwright tests. This catalog remains the authoritative ID and coverage index. The short **Actions** entries here are summaries, not manual procedures; do not infer missing operator gestures from them.

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

- **Starting show:** Load a fresh copy of canonical `compact-rig.show` for every intensity spread case and use Group 1 with ten ordered fixtures. The moving-head Pan encoder case loads a fresh copy of the Default Stage Show and uses Back Profile heads 101 through 105.
- **Surface:** Lightning Desk keypad/command line and the encoder value modal on both layouts — the software-only touch **Set value** dialog and the hardware-connected encoder modal share one THRU submission path — plus engine interpolation and real output.
- **Actions:** Through visible desk controls, enter `FIXTURE 1 THRU 5 AT 20 THRU 50`, complete a fixture selection and then enter `AT 0 THRU 50`, and enter `0 THRU 50` in the Dimmer encoder value modal on each layout. In both encoder value modals, also enter the multi-point expression `100 THRU 0 THRU 100` over the five-fixture ordered selection for the Dimmer encoder and for the Pan encoder. Also cover uniform `0`, descending `100 THRU 0`, and multi-point `100 THRU 0 THRU 100` intensity commands, live `GROUP` references, and dereferenced `DEGRP` fixture captures; reserve empty cases for color spreads.
- **Oracle:** Ordered normalized programmer values before quantization, address shape (group-relative spread versus fixture-scoped values), Fixture Sheet values, logical output, and matching Art-Net/sACN slots.
- **Pass:** Intensity spreads preserve target order, endpoints, direction, symmetry, and equal intervals. Multi-point encoder-modal entry follows the deterministic anchor rule — five ordered items resolve `100 THRU 0 THRU 100` to exactly `100, 50, 0, 50, 100` (`255, 128, 0, 128, 255` after DMX quantization) for intensity and Pan alike — and lands as one atomic programmer mutation with exactly one value per selected fixture. Live Group spreads recalculate when group membership changes and remain group-relative in Cue/Preset storage; dereferenced spreads stay attached to the captured fixtures. Unresolved center and complex-value rules remain explicitly documented rather than guessed.

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

### CROSS-003 — Relative encoders and immediate live timing

- **Starting show:** Fresh Default Stage Show and compact-rig copies with Programmer Fade at five seconds, including mixed and unpatched selections.
- **Surface:** Software touch encoder zones and Set Value, wheel/keyboard accessibility, hardware-connected display, physical/OSC encoder events, channel faders, typed WebSocket plus HTTP action mirror, Preset recall, and PRELOAD GO.
- **Actions:** Apply fine/coarse steps, continuous hold-drag, absolute Set Value, hardware/OSC turns, and channel fader movement; record and replay the resulting value; use Preset and PRELOAD as negative timing controls.
- **Pass:** Relative encoder and channel-fader output is immediate and carries no explicit `0s` recording override; absolute encoder Set Value follows **Direct entry uses Programmer Fade** exactly like command-line `AT`; mixed offsets clamp safely; a drag shares one undo entry; software and hardware feedback agree; Preset and PRELOAD transitions retain Programmer Fade.
- **Executable scenario:** [TIME-002](../../../tests/05-virtual-time-persistence-and-recovery.spec.ts), encoder component/application tests, and cross-surface encoder coverage.

### CROSS-004 — Attribute activation groups and Indexed Presets

- **Starting show:** A working show with two embedded fixture profiles whose compatible **Dots**
  function uses different raw DMX values, plus partial, incompatible, multi-channel control, and
  hazardous cases.
- **Surface:** Desk Setup attribute registry, six-slot encoder pages, software/hardware **Set
  Value**, typed Programmer HTTP/WebSocket actions, Fixture Sheet, Stage, and physical output.
- **Actions:** Configure and restore activation groups, program linked attributes, apply
  fixed/indexed rows, activate and release typed control rows, and replay stale selection/profile
  requests.
- **Pass:** Registry changes persist with the show; activation is server-owned and one Undo step;
  per-profile raw values resolve behind one semantic row; output is immediate within the Programmer
  tick contract; transient controls are atomic/non-recordable; unsafe or stale requests are
  rejected; numbered show Presets remain separate.
- **Executable scenario:** CROSS-004 in the cross-surface acceptance document, focused runtime and
  fixture tests, `AttributeRegistrySettings.test.tsx`, `indexedPresetChoices.test.ts`, and
  `ParameterControls.test.tsx`, plus root
  `tests/79-attribute-registry-indexed-presets.spec.ts`.

### PROG-001 — Selection persists through value entry until replaced or cleared

- **Starting show:** Load canonical `compact-rig.show`, immediately Save As `prog-001.show`, and use the active copy.
- **Surface:** Stage clicks and marquee, Fixture Sheet, Group UI, Lightning Desk command/value controls, encoders, and preset recall.
- **Actions:** Select fixtures and groups successively across surfaces, apply repeated values without reselecting, start a replacement selection, use leading `+` to continue a prior selection after a value edit, then press `CLR` once and select again.
- **Oracle:** Current programmer selection state, ordered source references, normalized deduplicated fixture targets, visible selection indicators, leading-plus continuation, and preserved programmer values.
- **Pass:** Selections accumulate across all surfaces as implicit `+` operations, value/encoder/preset edits leave the selection current, the next non-plus selection replaces the old targets, leading `+` continues them, and first-stage `CLR` clears only the selection.

### POSITION-HOME-001 — Return selected Position heads to profile homes

- **Starting show:** Load canonical `default-stage.show`, immediately Save As separate `position-home-001-api.show` and `position-home-001-ui.show` working copies, and use the active copy.
- **Surface:** Paired authenticated programmer command and the Position special dialog in both software-only and OSC hardware-connected desk layouts.
- **Actions:** Build a mixed ordered selection, establish prior Pan/Tilt programmer values, press **Return Home**, undo once, attach the simulated hardware surface, and press **Return Home** again.
- **Oracle:** Ordered per-head Pan/Tilt defaults with independent 50% fallback, skipped non-Position fixtures, one faded batch mutation and audit event, exact prior values after one Undo, and the same visible enabled action in both layouts.
- **Pass:** Return Home affects only compatible heads in the current selection, participates in ordinary programmer timing and Undo, and does not modify profile or recorded show data.
- **Executable scenario:** [POSITION-HOME-001](../../../tests/25-return-home-position-special-dialog.spec.ts)

### COLOR-RANGE-001 — Align an ordered range in the Color special dialog

- **Starting show:** Load canonical `default-stage.show`, immediately Save As separate `color-range-001-api.show` and `color-range-001-ui.show` working copies, and use the active copy.
- **Surface:** Paired authenticated batch command and the Color special dialog using normal keyboard Shift and attached OSC hardware Shift.
- **Actions:** Apply one uniform picker color, Undo, Shift-drag across the visible picker, prove no mutation before release and one mutation on release, Undo, cancel another active range, then repeat the completed range while attached hardware holds Shift.
- **Oracle:** Current ordered selection, exact straight-line hue/saturation interpolation at the displayed Brightness, RGB/CMY per-head assignments, skipped unsupported targets, visible start/end/line preview, batch audit count, and prior values after one Undo.
- **Pass:** Uniform color remains available, completed ranges apply once in selection order from either Shift source, and cancellation cannot leave partial programmer values.
- **Executable scenario:** [COLOR-RANGE-001](../../../tests/26-color-special-dialog-alignment.spec.ts)

### PLAYBACK-SELECT-001 — Hardware-connected playback card ownership

- **Starting show:** Load canonical `default-stage.show`, immediately Save As an isolated working copy, and use the active copy.
- **Surface:** The hardware-connected playback UI, including pointer, fader, button, Record, page-picker, Group, and OSC actions; API reads provide the authoritative oracle.
- **Actions:** Select a Cuelist card through its display-only label area, operate its real GO button and fader, Record to that card, select the same slot on another explicit page, return to page 1, select a Group card, and repeat Group selection through its attached-hardware button.
- **Oracle:** Desk-selected playback number, opened concrete Cuelist View, exact target Cuelist revision, unchanged other Cuelist, current programmer Group selection, persistent identity across page changes, and absence of nested representation buttons.
- **Pass:** The card owns one selection action, real controls remain independent outside Record, Cuelist and Group workflows stay distinct, Record makes the whole card target the concrete page/playback without operating its controls, and UI, API, and OSC converge on the same desk-local selected playback.
- **Executable scenario:** [PLAYBACK-SELECT-001](../../../tests/28-hardware-connected-playback-selection.spec.ts)

### CUELIST-LAYOUT-001 — Cuelist table density and authoritative phase progress

- **Starting show:** An isolated `compact-rig.show` copy with a three-Cue Sequence. Give the second Cue distinct Trigger Time, In Delay, In Fade, Out Delay, and Out Fade values so incoming and outgoing phases overlap, and store representative Cue preview pictures.
- **Surface:** Cuelist View as a full window and as fixed and follow-selection Cues panes in software-only and hardware-connected layouts.
- **Actions:** Inspect the default table, select rows without executing them, enable **Compact Cue rows**, run the timed Cue with the virtual clock, pause and resume during a phase, close and reopen the view after completion, then trigger the same Cue again.
- **Oracle:** Exact nine-column order and bounds; Preview-column and selected-Cue image visibility; authoritative trigger/transition identity; per-phase 0–1 progress applied to each timing cell background; playback paused state; and unchanged Cue-list revision/output during row selection.
- **Pass:** All default columns are simultaneously visible. Compact rows hide only table imagery and retain the selected-Cue preview. Empty timing backgrounds mean not started; Trigger Time begins with the actual trigger interval; In/Out delays and fades begin with their own phases and can overlap; pause freezes and resume continues each fill; completion remains filled across row, pane, and view changes; and only a new authoritative transition of that same Cue resets its cells.
- **Executable coverage:** Static table/editor behavior and selection-only safety are exercised by [CUE-011](../../../tests/06-cuelist-view-and-settings.spec.ts). The per-phase virtual-time case must use pushed authoritative runtime progress rather than reconstructing phase time in Playwright or the browser.

### COMMAND-HISTORY-001 — Inspect and reuse recent desk commands

- **Starting show:** Load canonical `default-stage.show`, immediately Save As an isolated working copy, and use the active copy.
- **Surface:** The production Command Line history UI across software, reconnect, hardware-connected, and OSC paths; API reads provide the authoritative oracle.
- **Actions:** Execute one accepted and one rejected command, open history without changing the unfinished input, inspect and explicitly reuse an entry, prove Enter remains required, close by button/Escape/outside press, reload, then execute through attached OSC hardware.
- **Oracle:** Desk-scoped newest-first entries with status, feedback, time, and source; exact current input; unchanged command-control geometry; panel bounds above the controls; retained same-process reconnect state; and one later entry only after reused Enter.
- **Pass:** Completed commands appear once with actionable context, inspection and dismissal are mutation-free, reuse is non-executing, the last 50 entries survive reconnect but not server restart, sensitive command terms are redacted, and all named input surfaces converge.
- **Executable scenario:** [COMMAND-HISTORY-001](../../../tests/30-command-line-history-panel.spec.ts)

### ENCODER-DISPLAY-001 — Mirror six attached hardware encoders

- **Starting show:** Load canonical `default-stage.show`, immediately Save As `encoder-display-001.show`, and use the active copy.
- **Surface:** Production hardware-connected Programmer and Stage feedback cards with attached OSC encoder input.
- **Actions:** Select a moving fixture, change families, turn and press-turn assigned encoders, inspect missing and discrete targets, and inspect Stage navigation/setup mappings.
- **Oracle:** Exactly six ordered cards, stable Enc 1–6 numbering, formatted authoritative targets, explicit turn/press-turn vocabulary, no slider semantics, desk-scoped OSC convergence, deterministic clearing, and non-overlapping measured bounds.
- **Pass:** The software surface truthfully mirrors the six physical targets without implying draggable faders, stale mappings, shifted gaps, clipped values, or cross-desk input.
- **Executable scenario:** [ENCODER-DISPLAY-001](../../../tests/31-hardware-connected-encoders.spec.ts)

### WORKFLOW-COLOR-001 — Distinguish Record and Update workflows

- **Starting show:** Load canonical `default-stage.show`, immediately Save As `workflow-color-001.show`, and use the active copy.
- **Surface:** Record Settings, Record target/choice surfaces, Update Settings, Update target/preview/result surfaces, and the global armed control in software-only and attached-hardware layouts.
- **Actions:** Open each settings workflow, inspect textual identity and choices, arm each mode, and repeat after attaching the OSC hardware surface.
- **Oracle:** Shared red Record and amber Update computed tokens/boundaries, explicit RECORD/UPDATE text, neutral Cancel, destructive Overwrite, independent error/disabled styling, focus/pressed states, and stable colors after the hardware layout activates.
- **Pass:** Operators can distinguish Record from Update without relying on color alone, while workflow color remains consistent and never masks destructive, disabled, Cancel, or error meaning.
- **Executable scenario:** [WORKFLOW-COLOR-001](../../../tests/33-record-and-update-menu-colors.spec.ts)

### PLAYBACK-COLOR-001 — Show authoritative configured playback color

- **Starting show:** Load canonical `default-stage.show`, immediately Save As `playback-color-001.show`, and configure a light-colored Cuelist playback on page 1.
- **Surface:** Software Playback controls, hardware-connected Playback controls, and Virtual Playbacks.
- **Actions:** Inspect the assigned inactive card, GO the playback through the authoritative runtime, select it independently, attach hardware, and inspect equivalent Virtual runtime projection.
- **Oracle:** Subdued versus strong configured-color computed treatment, runtime `running` state, separate cyan selected marker, readable light-color text, retained loaded/pickup/Swap/target markers, and no configured-color class on empty cells.
- **Pass:** Runtime state—not pointer history—controls configured-color strength consistently while selected and all combined states remain independently legible.
- **Executable scenario:** [PLAYBACK-COLOR-001](../../../tests/34-active-playback-colors.spec.ts)

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

- **Starting show:** Create and verify a new empty show before creating the page whose video becomes the maintained recording.
- **Surface:** The real `?demo=product` application, its simulated hardware keypad and playbacks, authenticated show-object setup, the manual clock, and logical DMX observation.
- **Actions:** Open Show Patch on the empty show; visibly add all three four-segment trusses through the Touch UI, keeping the first interaction at normal recording pace before compacting the repeated truss actions; fast-forward the stage deck, curtains, back and side railings, and vertical pipes through the API. Visibly patch and place the conventional front/side Fresnels, Profile Stage Center with one unaddressed physical multi-patch, House Lights with addressed multi-patches at `1.18`–`1.20`, and ACL Back Center with seven unaddressed physical multi-patches before fast-forwarding the remaining canonical lighting patch. During that fast-forward, select each destination layer before its fixture batch appears so the Patch view follows the work. Configure one output route visibly and the remaining routes behind a labelled fast-forward. Show `Setting up the Basics` while creating the dedicated Fixture Sheet/Group Pool desktop, then show `Defining Groups` immediately before the first Fixture Sheet selection. Select the 28 Beam Stage fixtures in approximately four seconds, type Group names through the Touch keyboard at the accelerated Group-name pace, retain the first pre-confirm hold, choose a canonical icon, and omit the long modal/name/save holds from the second naming pass. Compress Groups 3–9 into an approximately four-to-five-second simulated-hardware sequence. Announce Beam Stage plus Beam Audience before the narration hold, then visibly create Beam Show and its first odd/even derivatives before returning to the existing API fast-forward. Assign Group Masters by touch and command line. Open Presets once, reveal its in-place Group shortcuts, and use the Beam Show shortcut for every hand-programmed Position and Color example without switching back to the Group pool. Record the first two Color presets by hand, then compress the remaining eleven Color presets into approximately 3.5 seconds and give every Color preset a colored swatch icon. Keep the remaining Position and Beam generation at one second per preset, create the Profile Circle and PWM Dynamics and assign them to a new Virtual Playbacks desktop, then record the first ACL Cuelist through the keypad before completing the remaining canonical objects. Continue through Fixture Sheet, busking, Preload, and the ACL chaser on production operator surfaces.
- **Oracle:** The maintained video starts only after the empty show is active; the complete show contains 12 layers, 231 controllable lighting fixtures, 33 visual-only Venue records, 264 total patch records, 306 physical Stage instances, 2,988 occupied DMX slots, eight enabled Art-Net output routes, 35 Groups, 30 presets, 30 Dynamics, eight Cuelists, 14 Playbacks, and non-zero authoritative DMX output. Universe 1 contains conventional fixtures at `1.1`–`1.24`, Stage LED PARs from `1.25`, and the hazers at `1.509` and `1.511`, but no movers; Stage movers occupy universes 2–4, Audience fixtures 5–6, and Auxiliary fixtures universe 8.
- **Pass:** One Playwright test completes the full operator story without splitting state across scenarios. Run `npm run test:demo` on the operator computer to atomically refresh `assets/demo.show`, record the narrated Full HD product surface, and write the maintained screenshot and video below `.artifacts/test/visual-inspection/product-demo/`. Release CI instead runs `npm run test:demo-show`, which builds and validates the equivalent portable show through API requests without browser choreography or recording delays.
- **Executable scenario:** [DEMO-001](../../../tests/product-demo.spec.ts)

## BENCH-CLOCK-DMX-001–002 — Scenario clock and output observations

- **Starting show:** Use the isolated twelve-dimmer bench show and its test-owned Art-Net and
  sACN receivers; the free-run case installs one test-owned Dynamic source.
- **Surface:** Typed scenario clock, fixture-aware and raw logical-DMX observations, and decoded
  Art-Net/sACN packets.
- **Actions:** Render one zero-time step; visit named 999/1000/1001 ms boundaries; resolve
  fixture-number and fixture-range channel names from the current patch/profile; then run the
  production output scheduler against the manual clock for 350 ms of real recording time.
- **Oracle:** Exact application timestamps, one deterministic step, current semantic channel
  bytes, multiple changing Art-Net and sACN frames during free run, and no packets after the
  awaited operation freezes the clock.
- **Pass:** Exact tests never wait on wall time, fixture assertions never hard-code patch
  addresses or advance time, live recordings receive genuinely moving output, and logical,
  Art-Net, and sACN observations remain separate.
- **Executable scenarios:**
  [BENCH-CLOCK-DMX-001–002](../../../tests/testBench/04-clock-dmx-and-output.spec.ts)

## Highlight, Stage, and Matter proof boundaries

These three integrations span deterministic domain tests, browser semantics, and native or
network facilities that the browser bench does not provide. Report each layer separately rather
than treating one green browser scenario as proof of the whole boundary.

### Highlight

The executable browser roots in
[`57-semantic-system-integrations.spec.ts`](../../../tests/57-semantic-system-integrations.spec.ts)
cover visible HIGH stepping, external and empty selection, and the error overlay. Ownership and
alert geometry are registered directly by
[`11-update-highlight-fixture-profiles-and-matter.spec.ts`](../../../tests/11-update-highlight-fixture-profiles-and-matter.spec.ts).
The older `pairedScenario` declarations imported by that file are compatibility descriptions and
do not register tests; their broader output, Fixture Sheet, Stage, and geometry assertions must
not be reported as executed evidence.

Run the active deterministic layers with:

```sh
cargo test -p light-programmer highlight
cargo test -p light-headless-runtime --no-default-features highlight
cargo test -p light-fixture highlight
cargo test -p light-engine highlight
cd apps/light-desktop
npm test -- HighlightControls.test.tsx FixtureSheetWindow.highlight.test.tsx StageWindow.highlight.test.ts HardwareControlSummary.highlight.test.tsx
cd ../..
npm run test:e2e -- tests/57-semantic-system-integrations.spec.ts --grep HIGHLIGHT
npm run test:e2e -- tests/11-update-highlight-fixture-profiles-and-matter.spec.ts --grep 'HIGHLIGHT-004|HIGHLIGHT-005'
```

### Stage visualization

[`STAGE-001`](../../../tests/66-semantic-stage-visualizer.spec.ts) proves the strongest Stage
contract available to the browser bench: two operator-visible panes retain independent Live and
Follow Preload lane configuration, both remain 2D, and a browser without the native Tauri renderer
shows the explicit unavailable state instead of a client-local substitute canvas.

Deterministic transport and renderer tests separately cover bounded latest-frame delivery, lane
throttling, Live/Preload multiplexing, reconnect snapshots, stale-scope rejection, the four render
representations, retained scene resources, and Stage picking. Run those layers with:

```sh
cargo test -p light-headless-runtime visualization_transport
cd apps/light-desktop
npm test -- VisualizationRuntimeTransport.test.ts visualizationRuntime stage3dScene.test.ts stageWindow
cd ../..
npm run test:e2e -- tests/66-semantic-stage-visualizer.spec.ts
```

[`STAGE-PERF-001`](../../../docs/testing/16-stage-performance.md) and
[`STAGE-PERF-002`](../../../docs/testing/16-stage-performance.md) are implemented by the packaged
Tauri/WebView collector. They cover the default and deterministic 500-instance profiles and write
the stable `.artifacts/performance/stage/stage-visualization-timing.json` artifact in addition to
the timestamped report. Five-minute output and 30-minute resource gates require retained reports
from real supported-platform GPU runs; a short local run or browser/software renderer is explicitly
non-acceptance evidence.

### Matter playback bridge

Matter runtime tests cover stable explicit page/playback endpoint derivation, writes, tracking,
identity persistence, lifecycle, and filtering. Empty slots and unsupported target families are
omitted; an assigned control without a physical dimmable fader is intentionally exposed through
its authoritative virtual level and is not an omission case. The migrated browser root currently
proves only the persisted enable toggle. There is no direct `MatterBridgeSettings` component suite;
`DeskSettingsModal.test.tsx` tests Desktop clone/delete behavior and is not Matter evidence.

Run the active layers with:

```sh
cargo test -p light-headless-runtime --no-default-features matter
npm run test:e2e -- tests/57-semantic-system-integrations.spec.ts --grep MATTER-001
cargo test -p light-headless-runtime --no-default-features matter::transport::tests::commissionable_network_transport_smoke -- --ignored --test-threads=1
```

The ignored smoke requires host access to UDP 5540 and shared mDNS 5353. A real controller remains
the interoperability gate for fabric commissioning and subscriptions.

## Required coverage matrix

| IDs | Area | Required cases | Primary oracle |
| --- | --- | --- | --- |
| SHOW-000–005 | Show files and recovery | Save As copy isolation, save/restart/reopen, atomic replacement recovery, invalid-active-show recovery, stable legacy migration, Latest Autosave versus named revisions, timestamped revision-copy provenance, durable copy reopen/restart, Save As provenance preservation, alternate-destination confirmation, and recoverable overwrite | independent files and identities, visible provenance timestamps, hashes/revisions, restarted server, and the contents of preserved backup databases and corrupt files |
| DIM-001–002 | Foundational dimmers | ordered live Group editing and a visible Group command reaching logical, Art-Net, and sACN output | Group/programmer state and exact DMX |
| GROUP-003–005 | Group semantics | derived membership, frozen membership, stored empty Groups, skipped missing range IDs, unpatched members, and safe invalid references | Group objects, selection, visible panes, rendered output |
| PROG-001–004 | Programmer | selection gesture lifetime, ordered spreading, fixture/Group LTP, and two-stage Clear | programmer state, audit, panes, exact output |
| POSITION-HOME-001 | Position Return Home | ordered per-head profile defaults, independent 50% fallback, skipped incompatible fixtures, one faded Undo gesture, empty-selection safety, and software/hardware layout parity | paired programmer state, atomic batch audit, prior values after Undo, and production dialog controls |
| COLOR-RANGE-001 | Color range alignment | uniform click, ordered straight-line hue/saturation interpolation, current Brightness, RGB/CMY and logical-head resolution, reversed order, one-fixture endpoint, cancel safety, single release mutation, and software/hardware Shift | paired programmer state, batch audit counts, visible range overlay, exact prior values after Undo, and attached OSC Shift state |
| PLAYBACK-SELECT-001 | Hardware playback selection | single card-owned selection, display-only labels, independent real controls outside Record, whole-card concrete Cuelist Record/open behavior, Group selection, explicit page identity, and OSC convergence | paired selected playback, Cuelist revisions, Group programmer selection, rendered semantics, page picker, and attached OSC action |
| COMMAND-HISTORY-001 | Command Line history | accepted/rejected status, newest-first ordering, result/error/source/time context, 50-entry desk scope, sensitive-text redaction, non-executing reuse, deterministic dismissal, reconnect, and software/hardware geometry | paired history endpoint and panel, current input, entry count, server retention/redaction unit, production bounds, and attached OSC source |
| UPDATE-001–002 | Update | four Cue modes and authoritative tracked sources, exact eligibility, Preset/Group existing-versus-new semantics, touch/default/menu flows, current/explicit page context, pre-Update desk-settings migration with a schema-3 show, actual simulator pointer gesture exclusivity, and atomic revisions | paired and process-backed stored-object results, previews, revision history/undo, programmer retention, unrelated-object isolation, restart, simulator transport writes, and same-desk feedback |
| HIGHLIGHT-001 | Transient Highlight output | independent HIGH state, complete/singleton application, programmer/store isolation, fixture-look overrides, first-frame Off reveal, ownership, and safety/master behavior | paired Highlight/programmer state, stored objects, and resolved raw output |
| HIGHLIGHT-002 | Live selection source and lifecycle | Fixtures/Stage/Group/command selection reset, live Group ALL restoration, additive/subtractive stepped selection, multi-head/multipatch/unpatched/invalid items, empty selection, reconnect, and show-load clearing | paired actual selection, live source resolution, Highlight output, reconnect, and reload |
| HIGHLIGHT-003 | Corrected stepping controls, selection visualization, alerts, and surface-specific geometry | real-selection PREV/NEXT/ALL with wrap; independent exact-label HIGH; fixed four-key columns; software-only 2×2 Programmer Fade; blue blinking `Highlight` replacement for the DMX-rate label; immediate Stage beam feedback from the authoritative output selection; subdued Fixture Sheet base and prominent current step including multi-head rows and master-only contained state when subheads are hidden; top-layer dismissible errors; no separate command-bar/simulator Highlight panel; simulator two-row RECORD/Preload and adjacent equal-height Prog/Cue Fade; keyboard/REST/WebSocket/OSC/hardware parity; removed Capture actions | paired programmer/Highlight state, stored Groups, Fixture Sheet rows, production software/simulator geometry, Stage beam projection, alert reachability, absence of forbidden panels, and protocol responses |
| HIGHLIGHT-004 | Multi-user Highlight ownership | different-user same-desk exclusion, non-mutation on conflict, same-user session retention, last-session release, reacquisition, and other-desk isolation | authenticated session registry, Highlight owner/output state, programmer selection, HTTP conflict, and release lifecycle |
| HIGHLIGHT-005 | Production Highlight alert reachability | ownership and generic action failures at 1280×720 and 1600×1100 in software-only and hardware-connected layouts, above pane/modal content, with pointer and keyboard dismissal and invariant accepted geometry | production browser bounds, hit testing, focus, stacking, exact `HIGH` label, absence of a command-bar status panel, and before/after control geometry |
| HIGHLIGHT-006 | Hardware simulator geometry and faders | rendered RECORD/Preload and fixed key columns, adjacent full-height Programmer/Cue Fade, independent pointer transport, and top/bottom boundaries without a Highlight panel | production hardware-controls CSS/DOM geometry and captured Tauri control writes |
| FIXTURE-001–002 | Fixture profiles | desk-wide atomic revisions, editor/lookup/reorder, exact channels/functions/actions/color/geometry, focused physical metadata, equal-third notes/photograph/visualizer layout, confined photograph/icon/valid-GLB orbit preview and replacement, independent split patching, portable snapshots, GDTF retention, and v1 migration/recovery | paired profile/history, exact asset data, rendered and pointer-rotated GLB preview, immutable revisions, raw DMX, patch ranges, Stage geometry, restart snapshots, startup artifacts and warnings |
| FIXTURE-SHEET-001 | Fixture Sheet color boundary | dark, bright, absent, and mixed swatches at 1600×1100 software-only and 1280×720 hardware-connected layouts without changing dot or row geometry | production computed fill, border, dimensions, row bounds, selection styling, and attached pane screenshots |
| FIXTURE-SHEET-002 | Fixture Sheet programmable-row and compact-base contract | independent Venue, `visual_only`, and complete-`0.` exclusions; surviving ordinary and multi-head IDs; all eight authoritative base groups; Off/Icon only/Text only per-surface persistence; Dynamic identities beside stable bases while deterministic DMX changes; Preload, source, Group Flash/Highlight, unavailable and selection states; 430 px density and horizontal overflow | show/selection preservation, stable Fixture Sheet projection, changing logical/DMX-window output, exact settings labels, shared story renderer, row/cell bounds, generated screenshots, and restart-compatible desk state |
| FIXTURE-ADDRESS-001 | Fixture Address screen | authoritative complete-footprint availability, own-slot exclusion during moves, one integrated number block and 512-slot map, all split assignments, overlap/overflow rejection, explicit unpatch, confirm-once, Cancel/Close/Escape safety, and supported desk bounds | component mutation counts and patch payloads, production grid states, validation messages, and rendered dialog bounds |
| MATTER-001–002 | Matter playback bridge | desk-persistent enablement across shows/restart, active-show assignment reconciliation, global page/slot endpoint stability, omissions, OnOff/Level writes, bidirectional tracking, pairing/identity, transport lifecycle, and truthful failures | paired configuration/status, process-backed show/restart state, stable endpoint identity, adapter values, host-network production socket smoke, and subscription state |
| PRELOAD-001–006 | Preload | programmer-only blind values, physical action queue, virtual-playback pane/actions, all eight capture masks, atomic combined GO, programmer-only release | pending entries, one commit timestamp, playback runtime, rendered output |
| VPB-007 | Virtual Playbacks | inert software/hardware Shift selection and named zone editing; show-owned playback-number snapshots; overlap union; concurrent UI/REST/WS/OSC activation; two-desk shared arbitration across different layouts; explicit deletion; restart normalization | persisted zone store, serialized playback runtime, UI Settings, audit, rendered output |
| PBK-001–006 | Playback configuration | Set interception on every playback control, assignment/color/clear persistence, type-safe button and fader layouts, Cuelist actions, Master/X-fade/Temp, Flash/Temp LTP restoration, Swap protection, specialized masters | persisted playback definition, action verb, playback runtime, temporary ownership, master state, exact output |
| CMD-001–002 | Command line | fixture/Group default modes, ranges and dereferencing, plus Speed Group value/synchronization commands | visible command text, programmer state, audit, Speed Group state |
| CUE-001–014 | Cue/playback | record, tracking, cue-only restore, active-Cue deletion with held output/navigation, GO/back, Go To/Load, pause, release, per-value/Cue timing, GO/FOLLOW/TIME triggers, Cuelist View editing and transactional renumbering, Chaser/Speed Group settings with normalized crossfade percentage and legacy conversion, Intensity HTP/LTP, wrapping, First/Continue restart, timing bypass | playback state, persisted Cuelist data, exact virtual timestamps and 0/50/100-percent fade boundaries, UI selection without execution |
| CUELIST-LAYOUT-001 | Cuelist and Cue Settings layout | default `Preview`, `No.`, `Name`, `Trigger`, `Trigger Time`, `In Delay`, `In Fade`, `Out Delay`, `Out Fade` order with every column visible; optional compact table rows that hide row imagery; pointer/touch activation of every timing and trigger cell; one exact-property Cue modal with transactional Save/Cancel and no Cue sidebar; modal three-column Cuelist Settings with title-bar Mode/Renumber/Save; selection-only Cue table; authoritative phase backgrounds for Trigger Time and independent In/Out Delay/Fade; completed-fill retention, same-Cue retrigger reset, and pause/resume; object-level field ownership; dirty Save/Discard/Stay decision; and narrow software/hardware-connected reachability | production header/cell bounds, exact modal property identity, component Save/Cancel mutation counts, pushed per-phase runtime at exact 0/50/100-percent virtual-time boundaries, stable completed fills after view close/reopen, transition identity on reset, frozen/resumed progress, and persisted object revisions |
| SOUND-001 | Sound to Light | one desk/browser-local input selected in Desk Setup, ordinary tap tempo, Shift/hold settings entry, Manual/Speed Group/Sound to Light source state, recursion rejection, dirty-close choices, title actions, frequency/gain/confidence/smoothing/range/hold/ratio configuration, recorded 120 BPM analysis, authoritative mapping, manual fallback and ownership boundaries | persisted Speed Group config, desk-local device mapping, live analysis, authoritative Speed snapshot |
| MIB-001 | Move in Black | per-fixture enable/default, safety delay after resolved zero, future lit-position lookup, disabled comparison, cancellation, Cue-edit invalidation | patch persistence, normalized MIB runtime state, exact Position DMX boundaries |
| MERGE-001–003 | HTP/LTP | programmer priority/recency, programmer/playback arbitration, automatic full-overwrite release, and reversible Flash/Temp ownership | resolved source state and exact DMX |
| DMX-001–009 | Encoding/routes | single-byte values, ArtDMX/E1.31 fields and sequence, remapped/multiple routes, patch boundaries, 16-bit order/defaults, isolated output failure/recovery, minimum payloads, idle configured universes, disable-without-delete handoff, explicit Art-Net Broadcast/Unicast and sACN Multicast/Unicast destinations, equal payloads, migration, and resolved diagnostics | decoded real UDP datagrams, encoded destination captures, persisted routes, and route diagnostics |
| OSC-001–006 | Hardware OSC | feedback subscription, commands, subscriber isolation, invalid input, same-desk UI/OSC interaction state, desk isolation, and current/explicit page addressing | received OSC messages, command/audit state, UDP output |
| API-001–002 | REST/events | authentication, revision conflicts, CRUD, matching events, and audit ordering | HTTP status/body, events, audit |
| CROSS-001–002 | Cross-surface agreement | equivalent Group value through UI/API/OSC and visible UI synchronization after external mutation | normalized programmer/output state and visible UI |
| TIME-001–003 | Virtual time | zero tick, exact fade boundaries, chaser/Dynamic speed, pause/resume, and maximum one-week jump | exact virtual timestamp, runtime phase, and output frame |
| BENCH-CLOCK-DMX-001–002 | Test-bench clock and DMX helpers | exact steps and named boundaries, genuine scheduler free run, fixture/profile component resolution, repatch-safe semantic lookup, fixture ranges, raw logical frames, and distinct wire packets | application timestamp, latest logical frame, decoded Art-Net/sACN history, and diagnostic artifacts |
| FILE-001–002, FILE-016 | File Manager | confined revision-safe text, visible browse/edit, authenticated roots/capabilities/range streaming, file operations, configured roots, pane input ownership, and hosted picker contracts | HTTP status/body, persisted files, visible pane/picker state, OSC-owned input context |
| TEXT-001, TEXT-015 | Text Editor | file association and dirty state, multi-pane synchronization/conflicts, external updates, rename/delete recovery, read-only and Markdown modes | persisted text/layout and visible editor state |
| LOCK-001 | Desk Lock | synchronized multi-screen PIN/button locking, desk-scoped API and OSC suppression, stable output, and other-desk independence | lock API, visible dialogs, command line, DMX, OSC behavior |
| CLIENT-001 | Client history and removal | stable client identity, connected-first presence, last-connected restart persistence, legacy unknown timestamps, duplicate-free reconnect, active/self removal conflicts, scoped confirmed cleanup, and removed-client default re-registration | schema-v9 desk store, live session registry, production Choose default screen UI, process restart, unchanged users/show objects, and new default desk identity |
| DEMO-001 | Planned product demo | empty-show setup, 12-layer venue with 264 patch records and 306 physical Stage instances, visible and fast-forwarded output routing, hardware/touch Group setup, first-of-family Presets, Profile Circle/PWM Dynamics, Virtual Playbacks, ACL Cuelist programming, busking, Preload, and the Speed D ACL chaser in one narrated run | persisted show objects, visible product-demo controls, programmer/preload/playback state, authoritative DMX, Full HD video, and final screenshot |
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
