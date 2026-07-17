# Preload Modes and Virtual Playbacks

These scenarios define Preload as three independently configurable capture domains: programmer changes, physical/page-playback actions, and virtual-playback actions. Matching paired API/UI Playwright coverage lives in `tests/06-preload-modes-and-virtual-playbacks.spec.ts`.

## Common configuration contract

Settings exposes three independent switches:

- **Preload programmer changes**
- **Preload physical playback actions**
- **Preload virtual playback actions**

The default is the existing **Both** behavior: programmer changes and physical playback actions are enabled, while virtual playback actions remain disabled until the operator enables them. The three switches are persisted as independent booleans rather than one mutually exclusive mode, so all eight combinations are valid and must be tested.

While Preload is armed, an enabled domain is captured without changing its live target. A disabled domain behaves normally and remains live. Pressing **Preload GO** commits all captured domains at one application timestamp. The Programmer Fade master is the fallback transition time for the entire Preload GO, including values produced by physical and virtual playback actions; a playback's Cue Fade master is not substituted. Explicit per-value programmer timing remains authoritative unless the applicable Force Cue Timing rule says otherwise.

Flash is never captured. A Flash press and release execute live and leave no pending Preload action. Ordinary playback-master fader movement is also not a captured action. The captureable physical-playback actions are **GO**, **GO minus**, **ON**, **OFF**, **TOGGLE**, **TEMP ON**, and **TEMP OFF**.

A configured **TEMP** button keeps its normal press-to-toggle UI gesture; it does not become a held button. During capture, one TEMP press queues the explicit **TEMP ON** verb and the next press queues **TEMP OFF**, while the real temporary state remains unchanged until Preload GO.

Preload Release removes only the active temporary programmer contribution created by Preload GO. Physical and virtual actions have already executed against their real playbacks, so Release must not undo, rewind, switch off, or otherwise restore those playbacks.

## PRELOAD-001 — Programmer-only Preload is a blind temporary programmer

**Priority:** P0
**Primary layer:** Paired API/UI E2E

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `preload-001-programmer.show`, and use the active copy.

**Configuration:** Enable programmer changes. Disable physical and virtual playback actions.

**Detailed procedure:**

1. Set Programmer Fade to 3 seconds. Keep an unrelated playback active so restoration has an observable underlying source.
2. Arm Preload. Select Group 1 and enter Intensity 50% without an explicit time. Select Group 2 and enter Intensity 70% with `[TIME] [1]`.
3. Confirm the pending Preload programmer shows both values and their timing metadata, but live Stage values, Fixture Sheet current values, active playbacks, and DMX remain unchanged.
4. While Preload remains armed, press GO on a physical playback. Because physical playback capture is disabled, GO executes immediately and is not added to pending Preload.
5. Press **Preload GO** and mark that exact application timestamp.
   - Group 1 uses the 3-second Programmer Fade fallback.
   - Group 2 uses its explicit 1-second value time.
   - No playback action is replayed because none was captured.
6. At exact 1,000 ms and 3,000 ms checkpoints, assert programmer values, source ownership, and packets.
7. Invoke Preload Release by the documented long-press on Preload. Confirm only the active Preload programmer source disappears and the underlying live playback values return without a default frame.

**Pass condition:** Programmer-only Preload behaves as a blind programmer whose committed values form one releasable temporary source with explicit value timing and Programmer Fade fallback.

## PRELOAD-002 — Physical-playback-only Preload queues actions

**Priority:** P0
**Primary layer:** Paired API/UI E2E

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `preload-002-physical.show`, create two assigned multi-Cue Cuelists, and use the active copy.

**Configuration:** Disable programmer changes. Enable physical playback actions. Disable virtual playback actions.

**Detailed procedure:**

1. Set Programmer Fade to 2 seconds and deliberately set Cue Fade to a different value such as 7 seconds.
2. Arm Preload. Change a Group value in the programmer.
   - **Expect:** Because programmer capture is disabled, the change is live immediately and no pending programmer value is created.
3. Press physical/page-playback actions **GO**, **GO minus**, **ON**, **OFF**, and **TOGGLE** in independent cases. Press the configured **TEMP** button twice to produce **TEMP ON** followed by **TEMP OFF** without changing its press-to-toggle gesture.
   - **Expect before Preload GO:** Each action is shown in the ordered pending-action list; the real playback's enabled state, current Cue, master/temp state, and output do not change.
4. Press and release **FLASH** while Preload is armed.
   - **Expect:** Flash executes live while held, restores on release, and never appears in pending Preload.
5. Move a normal playback master fader.
   - **Expect:** The fader acts live and is not captured. Merely crossing zero must not synthesize a pending ON action.
6. Press **Preload GO**. Confirm every queued action executes against its real playback in recorded order at one application timestamp. Resulting value transitions use the 2-second Programmer Fade master rather than the 7-second Cue Fade master.
7. Invoke Preload Release. Confirm the playback states produced by the queued actions remain exactly as they were after Preload GO.

**Action semantics:** Pending entries store action verbs, not predicted end states. If a playback changes through another surface before Preload GO, queued GO, GO minus, and TOGGLE apply to its actual state at execution time. Multiple actions for one playback retain operator order; for example, two queued GO actions advance twice, while queued GO followed by OFF finishes Off.

**Pass condition:** Physical-playback-only Preload delays only the allowed action verbs, excludes Flash and fader movement, executes the queue with Programmer Fade, and is not undone by Preload Release.

## PRELOAD-003 — Virtual Playbacks are configurable single-button panes

**Priority:** P1
**Primary layer:** UI E2E plus server integration

**Implementation status:** Implemented with pane-native UI and server integration coverage.

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `preload-003-virtual.show`, and use the active copy.

**Virtual Playback UI contract:**

- **Virtual Playbacks** is available as a normal window/pane kind in the configurable pane system. It is never a built-in fixed surface.
- Each pane contains an operator-configurable grid of single-button cells.
- A cell can be assigned one Cuelist.
- The default cell action is **GO**.
- A cell can instead be configured as **TOGGLE**.
- The assignment, action, grid dimensions, and pane placement persist with the show/screen configuration.

**Detailed procedure:**

1. Add a **Virtual Playbacks** pane to an empty pane position. Configure a 2×2 grid.
   - Confirm the pane kind opens and renders the configured four cells.
2. Assign Cuelist 1 to cell 1 with its default GO action. Assign Cuelist 2 to cell 2 and change its action to TOGGLE.
3. With Preload disarmed, click both cells and prove they operate the same underlying Cuelists and playback engine as equivalent single-button physical playbacks.
4. Reload the show/screen and prove the pane geometry, grid, assignments, and actions persist.

**Pass condition:** Virtual Playbacks are configurable pane-native single-button playbacks, not a special built-in or a disconnected mock control.

## PRELOAD-004 — Virtual-playback Preload captures GO and TOGGLE

**Priority:** P1
**Primary layer:** Paired API/UI E2E

**Implementation status:** Implemented with paired API/UI coverage against the real Virtual Playbacks pane.

**Starting show:** Continue from an independent copy of the PRELOAD-003 arrangement.

**Configuration:** Disable programmer and physical playback capture. Enable virtual playback capture.

**Detailed procedure:**

1. Set Programmer Fade to 2.5 seconds and Cue Fade to 8 seconds. Arm Preload.
2. Change a programmer value on a fixture not controlled by either virtual playback, then press a physical playback GO button. Both execute live because their domains are disabled; the disjoint programmer value also proves its higher-priority live source cannot mask the two playback fade observations.
3. Click the virtual GO cell and virtual TOGGLE cell.
   - **Expect before Preload GO:** both actions appear in pending Preload, but neither underlying Cuelist/playback changes.
4. Press Preload GO. Both real playback actions begin at the same application timestamp and their resulting values transition using the 2.5-second Programmer Fade master, not Cue Fade.
5. Invoke Preload Release. Confirm neither virtual-triggered playback action is undone.

**Pass condition:** Virtual-playback capture delays pane actions until Preload GO and then executes them as real persistent playback actions using Programmer Fade.

## PRELOAD-005 — All eight capture-domain combinations are independent

**Priority:** P0
**Primary layer:** Settings persistence plus paired API/UI E2E

**Starting show:** Use a fresh `preload-005-<mask>.show` copy for each matrix row. Prepare one pending-capable programmer change, one physical GO action, and one virtual GO action with distinguishable fixture attributes.

| Programmer | Physical | Virtual | Before Preload GO | At Preload GO |
| --- | --- | --- | --- | --- |
| Off | Off | Off | All three actions execute live; pending Preload stays empty. | No-op. |
| On | Off | Off | Only programmer change is pending. | Commit programmer source. |
| Off | On | Off | Only physical action is pending. | Execute physical action. |
| Off | Off | On | Only virtual action is pending. | Execute virtual action. |
| On | On | Off | Programmer and physical action are pending. | Commit both together. This is the default **Both** configuration. |
| On | Off | On | Programmer and virtual action are pending. | Commit both together. |
| Off | On | On | Physical and virtual actions are pending. | Execute both together. |
| On | On | On | All three domains are pending. | Commit all three together. |

For every row, save and reload Settings before arming Preload and verify the exact switch mask persists. Disabled domains must remain live rather than being silently ignored. Enabled domains must remain unchanged live until Preload GO. All committed domains share one Preload GO application timestamp and the Programmer Fade master.

**Pass condition:** The three Settings switches are genuinely independent, persist correctly, and produce all eight capture combinations without hidden mutual exclusion.

## PRELOAD-006 — Combined Preload commits atomically and releases only programmer data

**Priority:** P0
**Primary layer:** Rust integration plus paired API/UI E2E

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `preload-006-combined-release.show`, and prepare one programmer value, one physical playback action, and one virtual playback action on non-overlapping fixture attributes.

**Configuration:** Enable all three domains.

**Detailed procedure:**

1. Arm Preload and enter a programmer value with no explicit time, queue physical playback GO, and queue virtual playback TOGGLE.
2. Confirm all three appear as distinguishable pending entries and none changes live output or playback state.
3. Press Preload GO and capture one application-time mark before execution.
4. Confirm the temporary programmer source and both real playback actions start from that same mark and use Programmer Fade. There must be no frame where only one domain has committed.
5. After all fades complete, invoke Preload Release.
   - The programmer Preload contribution is removed and its underlying source is revealed.
   - The physical playback remains at the Cue reached by GO.
   - The virtual playback remains in the state reached by TOGGLE.
6. Press Preload Release again. It is idempotent: no playback changes, no extra events, and no additional output transition occur.

**Pass condition:** Combined Preload is one atomic operator action, while Release has deliberately asymmetric ownership and removes only the temporary programmer contribution.

## VPB-007 — Named Virtual Playback exclusion zones are authoritative

**Priority:** P0
**Primary layer:** Paired UI/API E2E plus OSC and restart integration

**Implementation status:** Implemented in `tests/06-preload-modes-and-virtual-playbacks.spec.ts`. The focused contract covers the real pane, Settings, keyboard, REST, OSC, server serialization, overlapping zones, different-desk isolation, persisted configuration, and restored playback normalization.

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `vpb-007.show`, assign three distinguishable Cuelist playbacks to current-page cells 1–3, and use the active copy.

**Identity and persistence contract:**

- Membership stores one-based cells on one virtual-playback surface. The same cells apply to whichever playback page is current for that control desk.
- Configuration is scoped by show, control desk, and surface ID. Sessions on the same desk see one configuration. A second desk for the same user remains independent; this does not change the user's separately shared programmer values.
- Pane movement retains membership. Grid shrink retains out-of-range members as hidden/dormant Settings entries, and grid expansion restores them.
- Older layouts without zone data load an empty zone list and retain their cell assignments.

**Detailed procedure:**

1. Hold physical Shift and select cells 1 and 2 in the Virtual Playbacks pane.
   - **Expect:** Both cells show selection. Neither playback changes state, Cue, output, or activation timestamp.
2. Choose **Create Exclusion Zone**, name it `Front pair`, and confirm.
   - **Expect:** The named zone appears in Settings. Creation is inert even if both members were already On before confirmation.
3. Turn cell 1 On, then turn cell 2 On through the real pane.
   - **Expect:** Cell 2 wins and cell 1 turns Off. Turning cell 2 Off leaves both Off.
4. Repeat the activation order with the F1/F2 current-page keyboard shortcuts, authenticated REST actions, and OSC `/light/<desk>/page-playback/<cell>/button/1` messages.
   - **Expect:** Every path produces the same authoritative winner and output. No browser-local toggle is involved.
5. Change the desk to page 2, where the same cell positions contain different playback assignments, and operate cells 1 and 2.
   - **Expect:** The zone follows its stored cell positions on the desk's current page. The second cell's page-2 playback wins. Returning to page 1 restores the page-1 membership mapping.
6. Create a second zone containing cells 2 and 3. Activate cell 3 and then cell 2.
   - **Expect:** A cell may belong to multiple zones. Cell 2 releases the union of the other members, so cells 1 and 3 are both Off. Zone display order does not affect the result.
7. Submit simultaneous activation requests for cells 1 and 2.
   - **Expect:** The server serializes the actions and leaves exactly one member On. The last action processed by that boundary wins.
8. Rename the first zone, add cell 3, shrink the grid so cell 3 is hidden, reload, then reopen Settings.
   - **Expect:** Name, list order, visible membership, and retained hidden cell persist. The pane's row/column layout and playback assignments remain intact.
9. Attach another session for the same user to a different control desk and operate cells 1 and 2 there without configuring a zone.
   - **Expect:** Both may remain On. The first desk's zone is not a user-global button rule.
10. On the first desk, start cells 1 and 2 before creating the zone, create the zone without operating either cell, and restart the server.
   - **Expect:** Before restart both remain On because creation is inert. Before resumed output, restored state selects the latest activation; an exact timestamp tie uses the higher playback number.
11. Disable automatic full-override release on one member and repeat mutual exclusion, then turn the winner Off.
    - **Expect:** Exclusion still works, and no other member starts. Automatic full-override release and mutual exclusion remain independent.

**Pass condition:** Exclusion zones are named, editable, persistent desk/surface configuration; selection and editing are inert; every activation path reaches one atomic server rule; overlap, restart, page/cell identity, and desk isolation are deterministic.

## Follow-ups

| Scenario | Next tests after the primary case | First failure checks |
| --- | --- | --- |
| PRELOAD-001 | Add Color/Position values, explicit delay, and competing priorities. | Separate pending, active Preload, normal programmer, and playback contributions. |
| PRELOAD-002 | Queue actions across pages and change a target playback before Preload GO. | Inspect ordered action verbs and execution-time playback state. |
| PRELOAD-003 | Add resize/reflow, multiple panes, page changes, and invalid assignments. | Check pane persistence separately from Cuelist assignment. |
| PRELOAD-004 | Queue repeated virtual GO and TOGGLE actions. | Compare virtual-cell event order with actual playback revisions. |
| PRELOAD-005 | Change masks while pending data exists and define the confirmation behavior. | Inspect persisted settings and ownership of already-pending entries. |
| PRELOAD-006 | Combine overlapping attributes and playback auto-off behavior. | Compare the atomic commit timestamp before arbitration. |
| VPB-007 | Add large-zone ergonomics and intentional zone copy between desks. | Inspect surface ID, current desk/page, serialized activation audit, and hidden memberships. |
