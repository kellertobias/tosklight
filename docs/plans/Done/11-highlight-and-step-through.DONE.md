# Highlight and Step Through

## Status

**Foundation implementation status: Complete; interaction contract superseded.** Feature 11 delivered the transient Highlight output layer, per-fixture semantic Highlight Look, protection against stored Highlight data, ownership/session lifecycle, and safety/master behavior. Its original Capture, remembered-snapshot, non-wrapping step, and control-placement model is no longer authoritative. The completed corrective [Highlight Changes](20-highlight-changes.DONE.md) contract defines PREV, NEXT, ALL, independent HIGH, real-selection stepping, wrapping, live-source restoration, Fixture Sheet step visualization, surface-specific keypad/fader geometry, error alerts, forbidden status panels, and the replacement coverage.

## Operator intent

Highlight provides a temporary way to identify and work through fixtures without writing the identifying look into the programmer. The operator can select fixtures from the Fixture Sheet, Stage Sheet, or Groups, turn Highlight on, and see the selected fixture or fixtures turn on in their configured highlight look. The normal default is full intensity and white, but an individual fixture may instead be configured to use another useful color such as blue.

Highlight is especially useful while focusing or positioning several fixtures. PREV and NEXT step the actual programmer selection through a remembered live selection source, ALL restores that source's complete current membership, and HIGH independently applies the identifying look to whatever is actually selected.

## Highlight output is not programmer data

Highlight is a transient output layer, not a programmer value, preset, Cue value, or selection macro.

- Turning Highlight on must not add Intensity, Color, Beam, Position, or other values to the programmer.
- Highlight values must never be included by Record, Update, Merge, presets, Cues, or any other store operation.
- Normal values deliberately changed while Highlight is active still belong to the programmer and remain after Highlight is turned off.
- Turning Highlight off removes only the temporary highlight contribution and immediately reveals the correct underlying programmer/playback output.
- Clearing or releasing the programmer must not be required to remove Highlight.
- Highlight state is runtime operator state and must not be written into show programming or restored as live output merely because a show is saved and reopened.

The implementation therefore needs an explicit highlight contribution or diagnostic output layer. It must not simulate Highlight by silently putting values into the programmer and later trying to remove them.

## Fixture-level highlight look

Each fixture needs a persisted Highlight Look setting that describes how that fixture identifies itself. This belongs to the fixture, independently of its DMX address or other patch-address details. A fixture-type/profile default may be useful, but an operator must be able to override the look for an individual fixture.

The default semantic look is:

- Intensity/Dimmer at full;
- white on fixtures that can produce color; and
- no unnecessary change to Position or other attributes.

The configuration must also support a different identifying color, such as blue. Planning must define how this semantic color is represented across RGB, RGBW, CMY, color-wheel, conventional dimmer, and other fixture capabilities. Some fixtures may also require an open shutter or another fixture-specific value before they emit light; the Highlight Look must be capable of describing those required values without turning them into programmer data.

Old shows without a Highlight Look must receive a safe, deterministic default. Copying or duplicating a fixture should preserve its configured look. Before implementation, decide whether this setting is stored directly on the show fixture, inherited from the fixture definition with a per-fixture override, or represented by another fixture-owned model. It must not be coupled to universe/address assignment.

## Corrected selection and stepping model

The original Feature 11 capture model is withdrawn. The corrected model keeps three concepts separate:

1. the actual ordered programmer selection used by Presets, encoders, dialogs, Group storage, and every other programming operation;
2. the remembered live selection source used by PREV, NEXT, and ALL; and
3. whether HIGH is active.

From the complete selection, NEXT remembers its live source and makes the first item the actual selection; PREV starts at the last item. Further NEXT or PREV actions wrap through the current valid order. ALL re-resolves the live source, restores its complete current ordered membership, and leaves the singleton step position. An external authoritative selection replaces the old step basis and becomes the new complete selection. Programmer-value and Preset changes do not reset the basis.

HIGH neither captures nor steps. It applies the Highlight Look to exactly the actual current selection, including every member in complete state or the singleton in step state. It may remain active across an empty selection and applies immediately when a later selection becomes non-empty. Turning it off removes only its transient output; it does not restore ALL, clear selection, or remove programmer values.

Stepping follows ordinary ordered-selection behavior, including multi-head expansion, first-occurrence duplicate handling, Group order, additions/removals, and live Group re-resolution. A multipatched logical fixture or head remains one item. Unpatched items remain selectable without output. Invalid items disappear from the current live sequence deterministically, including the empty-sequence case.

The Fixture Sheet is the normal visual source for the two selection layers while stepping. Every remembered-base fixture or head remains subtly selected, the actual current step is more prominent with a non-color distinction, and collapsed parents expose contained base/current state. This remains visible independently of HIGH; ALL restores ordinary complete-selection styling and an external selection replaces both indications.

## Surfaces and controls

Fixtures, Stage, Groups, and command-line selection all feed the same authoritative selection and external-selection reset behavior. The corrected control set has exactly four actions: HIGH, PREV, NEXT, and ALL. There is no Capture action.

On both software-only and hardware-connected num blocks, HIGH, PREV, NEXT, and ALL form one horizontal row directly above GRP, CUE, TIME, and DIV in those exact columns. HIGH contains only its centered label, uses the ordinary neutral key treatment while inactive, and uses the SHIFT/SET active treatment while Highlight is active. Keyboard bindings are `Alt+H`, `Alt+Left`, `Alt+Right`, and `Alt+A`; `Alt+C` has no Highlight action.

The software Programmer Fade occupies two button columns by two complete button rows. The hardware simulator uses the corresponding command-grid area for RECORD and PRELOAD GO, one column and two rows each, while equal full-height Programmer Fade and Cue Fade faders remain directly beside each other in its fader area. The command bar contains no Highlight status menu or summary, and the simulator has no dedicated Highlight display. Normal state is communicated by HIGH and the main desk's Fixture Sheet; actionable failures use a dismissible alert above panes and modal surfaces without changing the grid or HIGH size.

Software, keyboard, REST, WebSocket, OSC, and attached hardware invoke one authoritative selection, step, and HIGH state. Feedback independently exposes HIGH active, complete-versus-step mode, active index and total, and active fixture/head identity.

## OSC and hardware control

Highlight and stepping require OSC input so a physical controller can provide dedicated buttons. The OSC surface supports Highlight On, Off, Toggle, Next, Previous, and ALL; Capture and Reset are removed. It exposes feedback for:

- whether Highlight is active;
- the active step index and total number of valid fixtures;
- the active fixture identity/number and name;
- complete-versus-step mode; and
- whether Next and Previous are available for a non-empty live source.

The OSC contract must use the same user/session state as the software controls. Repeated messages, reconnects, and simultaneous software/hardware operation must not advance twice or cause the clients to disagree about the active fixture.

## Runtime ownership and safety decisions

Highlight belongs to the operator/session rather than to a playback. If the same user is connected from multiple devices, every device should see the same Highlight and step state; a different user must not silently take over that state.

For attributes in the fixture's Highlight Look, Highlight remains above normal programmer, playback, Group Master, and other ordinary contributions. Grand Master remains above Highlight and is applied exactly once. Blackout suppresses applicable Highlight output, disabled routes remain disabled, Blind/Preview/Preload suppress live output while keeping state inspectable, and hazardous-fixture safe values remain above Highlight where required. The desk has no separate parked-output source, and Highlight neither creates nor clears parked data.

Highlight changes snap immediately. Turning it off reveals the correct underlying programmer or playback winner in the next frame without an intervening default frame. These safety and priority rules are retained by Feature 20 while its corrected controls change the actual selection semantics.

## Required implementation contract and tests

The retained Feature 11 foundation coverage proves at least:

1. Highlighting a multi-fixture actual selection from the Fixture Sheet, Stage Sheet, and a Group.
2. Default full/white behavior and an individual fixture configured to identify in another color.
3. Highlight output never appearing in programmer state or any recorded object.
4. Normal Position changes remaining in the programmer while PREV and NEXT change the actual singleton selection.
5. NEXT, PREV, and ALL preserving the remembered live source across programmer-value changes and re-resolving its current membership.
6. Removal or invalidation of a fixture during stepping without corrupting the remaining sequence.
7. Turning Highlight off revealing the correct underlying programmer/playback values without a transient default frame.
8. Software, keyboard, REST, WebSocket, OSC, and attached hardware sharing authoritative state and feedback without double advances.
9. User/session isolation, reconnect behavior, and save/reload behavior.
10. The chosen Grand Master, Blackout, Blind/Preview, priority, and safety semantics.

Feature 20 completes the replacement real-selection, wrap, live-source, external-reset, Fixture Sheet base/current visualization, surface-specific software/simulator geometry, top-layer alert, forbidden-status-panel, keyboard, protocol-removal, and feedback acceptance cases. Operator help, protocol documentation, hardware mappings, and executable coverage were updated together under the completed corrective contract.
