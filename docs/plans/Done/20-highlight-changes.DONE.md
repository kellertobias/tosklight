# Highlight Changes

## Status

**Implementation status: Complete.** This corrective feature supersedes the capture, remembered-selection, stepping, and control-layout semantics in the completed [Highlight and Step Through](11-highlight-and-step-through.DONE.md) foundation. PREV, NEXT, ALL, independent HIGH, real-selection stepping, wrapping, live-source restoration, Fixture Sheet base/current visualization, software and restored hardware-simulator geometry, top-layer errors, protocol feedback, and replacement coverage are implemented together. The existing transient Highlight output layer, fixture Highlight Look, protection against recording Highlight values, operator/session ownership, and safety behavior remain unless this document explicitly changes them.

The corrected operator model has exactly four actions: **PREV**, **NEXT**, **ALL**, and Highlight, whose visible button label is **HIGH**. There is no separate Capture action. Stepping changes the real programmer selection, while Highlight independently applies the configured Highlight Look to whatever is actually selected.

## Selection and programmer state

The desk must keep the following concepts separate:

1. the actual ordered programmer selection on which presets, encoders, special dialogs, Group storage, and other programming operations act;
2. the remembered live selection source used by PREV, NEXT, and ALL while stepping; and
3. whether Highlight is active.

PREV, NEXT, and ALL change the actual programmer selection. They do not clear or otherwise change the programmer's existing attribute values. Values already programmed for previously selected fixtures or heads remain active in the programmer, while any subsequent value assignment acts only on the actual current selection.

For example, after NEXT has reduced a four-fixture selection to fixture 2, applying a Position Preset or moving a Position encoder affects fixture 2 only. Recording a Group while stepped records the actual singleton selection. Pressing ALL first restores the complete selection, after which Group storage uses that restored selection.

Highlight values themselves remain transient output. They must never become programmer values or be included by Record, Update, Merge, a Group, a Preset, a Cue, or another stored object.

## PREV, NEXT, and ALL

When the operator is in the complete selection rather than on one stepped item:

- **NEXT** remembers the current live ordered selection source, changes the actual programmer selection to its first item, and enters step mode.
- **PREV** remembers the current live ordered selection source, changes the actual programmer selection to its last item, and enters step mode.
- **ALL** leaves the already-complete selection unchanged if there is no remembered step source.

While step mode is active:

- **NEXT** selects the next item in the remembered order.
- **PREV** selects the previous item in the remembered order.
- NEXT from the last item wraps to the first item.
- PREV from the first item wraps to the last item.
- **ALL** re-resolves the remembered live selection source, restores its complete current ordered membership as the actual programmer selection, and exits the single-item step position.

ALL does not turn Highlight on or off. After ALL, another NEXT starts at the first item and another PREV starts at the last item.

The remembered state must preserve the selection expression or source, not only a snapshot of resolved fixture IDs. If stepping began from a live Group and that Group is edited before ALL is pressed, ALL restores the Group's current ordered membership.

## Selection changes reset the step basis

Any selection operation not caused by PREV, NEXT, or ALL replaces the previous remembered step source with the resulting actual selection and returns the operator to the complete-selection state. This applies equally to selection from Fixtures, Stage, Groups, the command line, or another authoritative control surface.

The reset is driven by selection changes, not programmer-value changes. Applying a Preset, moving an encoder, using a special dialog, or otherwise changing attributes must not replace the remembered selection source.

The implementation must distinguish its own PREV, NEXT, and ALL selection writes from external operator selection changes. Otherwise selecting a single step item would accidentally discard the complete remembered source.

An additive or subtractive selection gesture while stepped operates on the actual singleton selection according to the desk's normal selection rules. The resulting actual selection becomes the new complete selection and the new basis for later stepping.

For example, if Group 1 selected fixtures 1–4 and the operator stepped to fixture 2, then selected Group 2 containing fixtures 3–6, the actual selection becomes fixtures 3–6 and the old 1–4 step source is discarded. NEXT then selects fixture 3; PREV from the complete 3–6 selection selects fixture 6.

## Highlight behavior

**HIGH** is an independent Highlight On/Off toggle. It does not capture a selection, start stepping, restore ALL, or alter the programmer selection.

When Highlight is active, the configured Highlight Look applies to exactly the actual current selection:

- in the complete-selection state, every selected fixture or head is highlighted;
- in step mode, only the currently selected fixture or head is highlighted;
- NEXT or PREV removes Highlight from the old step item and applies it to the newly selected item;
- ALL applies Highlight to the complete restored selection; and
- an external selection change immediately moves Highlight to the new complete selection.

Highlight may remain active while the selection is empty. In that state it produces no Highlight output. A later non-empty selection receives Highlight immediately without requiring the operator to toggle Highlight again.

The HIGH control must visibly indicate whether Highlight is active, independently of whether the selection is complete, stepped, or empty. PREV, NEXT, ALL, and Highlight must use the same authoritative state across software, keyboard, REST, WebSocket feedback, OSC, and attached hardware.

## Fixture Sheet selection visualization

The Fixture Sheet must keep the complete remembered step selection visible while PREV or NEXT has changed the actual programmer selection to one fixture or head. The operator must be able to see both the base they are stepping through and the current sub-selection without opening another status view.

While stepping:

- every fixture or head in the remembered base selection retains a selected appearance, but uses a deliberately subdued or dimmed selection treatment;
- the fixture or head in the actual current programmer selection uses the prominent selection treatment; and
- PREV or NEXT moves that prominent treatment to the new step item while the subdued base remains visible.

The active step may be distinguished through stronger fill, contrast, or an additional bar at the left of its Fixture Sheet row. The exact ornament may follow the established Fixture Sheet visual language, but the active step must be immediately distinguishable from the subdued base and from fixtures that are not part of the selection. The distinction must not rely on color alone.

This visualization represents selection and stepping, not whether HIGH is active. It remains visible while stepping with Highlight off. Turning HIGH on or off must not remove or change the remembered-base indication. Pressing ALL restores the complete selection to the normal prominent selected treatment and removes the separate subdued-base/active-step distinction. An external selection change replaces both indications with the new complete selection, following the normal reset behavior.

For multi-head fixtures, the Fixture Sheet must show the state on the actual selected head rows: all heads in the remembered base receive the subdued treatment and the current stepped head receives the prominent treatment. Collapsed parent rows must still provide a visible indication that they contain the active step or another remembered-base member.

## Multi-head fixtures and ordering

Stepping consumes the desk's authoritative ordered selection exactly as normal programming does. Selecting a multi-head fixture already expands it into its selectable non-master heads; each resulting head is an ordinary independent step item. Highlight must not add a second master-head rule or treat a selected compound fixture as one special step identity.

Normal ordered-selection behavior continues to define Group order, ranges, additions, removals, and duplicate handling. Unpatched selected fixtures and heads remain valid selection and step items even though they produce no DMX until patched. Multipatch behavior remains tied to the logical selected fixture or head rather than creating a separate step for every physical patch copy.

If an item is deleted or otherwise becomes invalid during stepping, it is removed from the resolved sequence without corrupting the remaining order. Stepping continues through the remaining valid items. If no valid items remain, the actual selection becomes empty; Highlight may remain active but emits no Highlight output.

## Output priority and safety

For the attributes defined by the fixture's Highlight Look, Highlight has priority over normal programmer values, playback values, Group Masters, and other ordinary value contributions. Grand Master remains above Highlight and is applied exactly once.

Explicit safety and output-suppression behavior also remains authoritative:

- Blackout suppresses the applicable Highlight output;
- disabled output routes remain disabled;
- Blind and Preview suppress live Highlight output while still allowing the operator state to be inspected or prepared; and
- hazardous-fixture safe values remain above Highlight where required to prevent unsafe output.

Turning Highlight off removes only its transient contribution and reveals the correct underlying programmer or playback winner in the next frame without an intervening default frame. It does not restore ALL, clear the selection, or remove programmer values.

## Control and protocol changes

Every control surface must expose the same four actions and feedback:

- **PREV**;
- **NEXT**;
- **ALL**; and
- **HIGH** for Highlight On/Off or Toggle.

### Keypad placement

On the software num block and the matching attached-hardware layout, the four buttons form one horizontal row directly above the existing **GRP**, **CUE**, **TIME**, and **DIV** row. Their columns are fixed:

- **HIGH** is directly above **GRP**;
- **PREV** is directly above **CUE**;
- **NEXT** is directly above **TIME**; and
- **ALL** is directly above **DIV**.

The buttons must follow those exact columns rather than appearing as a detached Highlight toolbar or being reordered to match the prose action list. The same geometry must be preserved in software-only and hardware-connected num-block layouts.

HIGH is one ordinary button in the regular num-block grid. It occupies exactly one grid cell and has the same width, height, gap, border, typography, and alignment as PREV, NEXT, ALL, and the GRP/CUE/TIME/DIV buttons below it. It must not be wider, span columns, contain a nested status layout, or appear as a separate Highlight toolbar.

The button contains only the centered label **HIGH**. It must not show `No capture`, Capture, a captured count, fixture details, selection mode, or any secondary status text. When Highlight is inactive, HIGH uses the num block's normal unlit/off key treatment, like an inactive CLR key. When Highlight is active, HIGH is visibly lit using the same armed/active treatment as SHIFT or SET. The lit state follows Highlight itself even when the current selection is empty or live output is safety-suppressed; any additional suppression or ownership feedback belongs outside the button.

There is no separate Highlight status menu or panel between the command line and the REC/Preload controls. That command-bar space must remain free of Highlight selection, step, capture, and suppression summaries. Normal state is communicated by the lit HIGH key and the Fixture Sheet selection visualization; actionable errors use the dedicated alert described below.

In the software Programmer num block, the **Programmer Fade** control returns to a height of two complete button rows. It remains two button columns wide, forming a two-by-two-button-sized control rather than the current single-row-high strip. This is the chosen software layout; do not leave an additional empty button row beneath a one-row Programmer Fade control. The next button row follows after the normal num-block grid gap.

The hardware simulator uses that same two-column-by-two-row command-grid area for **RECORD** and **PRELOAD GO**, not Programmer Fade. RECORD occupies the left column for both rows and PRELOAD GO occupies the right column for both rows. Neither button sits in a detached row above the num block. In the simulator's fader area, **Programmer Fade** is restored as a regular full-height vertical fader directly beside the full-height **Cue Fade** fader. The two faders have equal geometry and remain simultaneously visible and operable.

The hardware simulator has no dedicated Highlight status display, selection summary, or output-suppression panel. Its Highlight UI consists only of the regular HIGH, PREV, NEXT, and ALL grid buttons, including the lit state of HIGH. Fixture/step state remains available on the main desk's Fixture Sheet and through protocol feedback without consuming simulator layout space.

The current Capture concept must be removed from the operator UI, keyboard shortcuts, REST action enum, OSC input, hardware mapping, help, and feedback. The existing ALL button must perform the restoration behavior specified here rather than dispatching Capture under another label. Any current `Alt+C` Capture shortcut is removed; final keyboard bindings for PREV, NEXT, ALL, and Highlight must be documented consistently with the software and hardware labels.

Feedback must expose at least whether Highlight is active, whether the operator is in the complete-selection or single-step state, the active step index and total, and the active fixture or head identity. Because stepping wraps, PREV and NEXT remain available whenever the remembered source resolves to at least one valid item.

Highlight errors must appear in a clearly visible, dismissible alert owned by the num-block controls. The alert may overlay the desk without changing the num-block grid, but it must render above all pane content, modal surfaces, and neighboring control sections rather than floating underneath or being clipped by another window. It must remain fully readable and reachable at supported software-only and hardware-connected window sizes. Error feedback must not be inserted into or widen the HIGH button.

These changes affect transient control state only. No persisted show migration is required, and saving or reopening a show must not restore live Highlight output or a stale step position.

## Required implementation contract and tests

Before this correction is considered complete, update the implementation, operator help, protocol documentation, hardware mappings, and executable acceptance coverage together. Coverage must include at least:

1. Select fixtures 1, 2, 3, and 4. NEXT selects 1; another NEXT selects 2; ALL restores all four; PREV selects 4; subsequent PREV presses select 3 and then 2.
2. NEXT from the last item wraps to the first, and PREV from the first wraps to the last.
3. Step to fixture 2, turn Highlight on, and verify the control visibly becomes active. NEXT selects and highlights fixture 3 while fixture 2 immediately loses its Highlight Look.
4. While Highlight is active, replace the selection with a Group containing fixtures 3, 4, 5, and 6. Verify all four become the actual selection and are highlighted. NEXT then selects and highlights fixture 3; PREV from ALL selects and highlights fixture 6.
5. Program distinct Position values while stepping through several fixtures. Verify earlier values remain in the programmer and each new value or Preset applies only to the currently selected fixture.
6. Record a Group while stepped and verify it contains only the actual singleton selection. Press ALL, record another Group, and verify it contains the complete restored selection. Neither stored object contains Highlight data.
7. Begin stepping from a live Group, edit that Group's membership, press ALL, and verify the Group's current ordered membership is restored rather than the original resolved snapshot.
8. Select a multi-head fixture and verify PREV and NEXT step through every selected non-master head in authoritative order as regular selection items.
9. Clear the selection while Highlight is active, verify output is removed without deactivating Highlight, then make another selection and verify its members are highlighted automatically.
10. Verify Blackout, Grand Master, disabled output, Blind/Preview suppression, hazardous safe values, and Highlight-off reveal behavior at the first affected output frame.
11. Remove active and inactive members while stepping and verify the remaining live sequence, wrap behavior, ALL restoration, and empty-sequence behavior remain deterministic.
12. Verify software, keyboard, REST, WebSocket, OSC, and attached hardware share one authoritative selection, step state, Highlight state, and feedback without double advances or client-local captures.
13. Verify the software-only and hardware-connected num blocks place HIGH, PREV, NEXT, and ALL in one row directly above GRP, CUE, TIME, and DIV respectively, with each pair aligned in the same column.
14. Verify the software Programmer Fade occupies exactly two button columns by two button rows, followed by only the normal grid gap rather than an empty button row, with the complete control remaining readable and operable.
15. Verify HIGH occupies exactly one normal num-block grid cell with the same dimensions and styling as the neighboring keys, contains no text other than `HIGH`, is unlit when Highlight is inactive, and uses the same visibly lit active treatment as SHIFT or SET whenever Highlight is active, including with an empty selection or safety-suppressed output.
16. Trigger every Highlight error in software-only and hardware-connected layouts and verify the dismissible alert remains fully visible and interactive above all panes, windows, and neighboring controls without changing the num-block grid or HIGH button size.
17. In the Fixture Sheet, step through a multi-fixture and multi-head selection with HIGH both off and on. Verify every remembered-base row remains visibly but subtly selected, the current step is more prominent and moves with PREV/NEXT, collapsed parents expose contained state, ALL restores normal complete-selection styling, and an external selection change replaces both indications.
18. Verify no Highlight status menu, selection summary, or suppression panel appears between the command line and the REC/Preload controls, and no dedicated Highlight display appears anywhere in the hardware simulator.
19. In the hardware simulator, verify RECORD and PRELOAD GO occupy the two-column-by-two-row command-grid area as one-column-by-two-row buttons, while Programmer Fade and Cue Fade remain equal full-height faders directly beside each other.
