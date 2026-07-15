# Foundational Dimmers and Groups

These scenarios use `compact-rig.show` and form the fastest confidence suite. Unless noted otherwise, assertions inspect application state and both real UDP outputs.

## How to run this file

Before every scenario variant, load canonical `compact-rig.show`, immediately use Save As with a unique filename derived from the scenario ID and surface, and use only that active working copy. The `@api` variant avoids browser startup; the `@ui` variant opens the embedded production UI. Before each action, capture the current revision and both UDP receiver marks. After the action, wait for the programmer or object revision, advance to the stated virtual timestamp, and run the same normalized state and Art-Net/sACN assertions for both variants.

## DIM-001 — Ordered group edits remain live

**Priority:** P0
**Primary layer:** Playwright E2E

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `dim-001.show`, and use the active copy for this scenario.

**Setup:** Group 3 contains fixtures 1, 2, 3, and 4 in that order. No programmer values are active.

**Actions:**

1. Open **Groups** and click Group 3 once. Do not double-click it; the selection must remain a live Group reference.
2. In the intensity control, enter `50` and confirm the value. Group 3 remains visibly selected and still current. Confirm that the programmer shows one Group 3 intensity value rather than four fixture values.
3. Advance virtual time by exactly 3,000 ms through `POST /api/v1/test/clock/advance` with `{"millis":3000}`. Treat the returned frame and packets received after the pre-action marks as the first output checkpoint.
4. Exercise each add-to-end workflow from a fresh `dim-001` working copy; all three must produce ordered membership `1, 2, 3, 4, 5, 6` while the original Group 3 programmer value remains active:
   1. **Merge workflow:** click fixture 5, then click fixture 6. Do not hold a modifier. Press `[REC]`, click Group 3, and choose **Merge** in the existing-Group recording dialog.
   2. **Live-reference overwrite workflow:** click Group 3, click fixture 5, and then click fixture 6. Do not apply a value between those clicks. Press `[REC]`, click Group 3, and choose **Overwrite** in the existing-Group recording dialog. The resolved ordered selection `Group 3 + 5 + 6` must be stored as `1, 2, 3, 4, 5, 6`, not as a self-referencing Group.
   3. **Command-line merge workflow:** click fixture 5 and then fixture 6 without holding a modifier. Verify the open ordered selection is `5, 6`. Press `[REC]`, `[+]`, `[GRP]`, `[3]`, `[ENTER]` in exactly that order. Confirm the displayed command is `RECORD + GROUP 3` before Enter, no confirmation dialog opens, and Group 3 becomes `1, 2, 3, 4, 5, 6`.
5. Continue the primary run from either successful workflow. Press `[GRP] [3] [-] [2] [ENTER]`. Confirm that the resolved selection is `1, 3, 4, 5, 6`, with fixture 2 removed and the relative order of every retained member unchanged.
6. Press `[REC]`, click Group 3, and choose **Overwrite**. Confirm that the Group card/context and API report ordered membership `1, 3, 4, 5, 6`. Do not touch the intensity control. Advance virtual time by `0` ms and use the emitted frame as the removal checkpoint: fixtures 1, 3, 4, 5, and 6 remain at the live Group value while fixture 2 falls to its unrelated/default value.
7. Press `[GRP] [3] [+] [2] [ENTER]`. Confirm that the resolved selection is `1, 3, 4, 5, 6, 2`. Because fixture 2 was removed before this new addition, it is appended at the end; it does not return to its former second position.
8. Press `[REC]`, click Group 3, and choose **Overwrite**. Confirm that the Group card/context and API report `1, 3, 4, 5, 6, 2`. Do not touch the intensity control. Advance virtual time by `0` ms and use the emitted frame as the re-addition checkpoint.
9. From another fresh `dim-001` working copy after step 4 has produced Group order `1, 2, 3, 4, 5, 6`, press `[GRP] [3] [-] [2] [+] [2] [ENTER]` as one command. Confirm left-to-right evaluation: fixture 2 is removed from its original position and then appended, producing `1, 3, 4, 5, 6, 2`. Press `[REC]`, click Group 3, choose **Overwrite**, and assert the same stored order through the Group UI and API.
10. From another fresh copy where Group 3 is `1, 2, 3, 4, 5, 6`, click fixture 2 once so it is the only open selection. Press `[REC]`, `[-]`, `[GRP]`, `[3]`, `[ENTER]`. Confirm the command line reads `RECORD - GROUP 3` before Enter, no dialog opens, and Group 3 becomes `1, 3, 4, 5, 6`. Repeat with fixtures 5 and 6 selected in that click order and confirm both are removed while the retained order stays `1, 3, 4`.
11. Prove delete equivalence with two independent fresh copies. In copy A, press `[CLR]` until the selection is empty, without creating a new selection; then press `[REC]`, `[-]`, `[GRP]`, `[3]`, `[ENTER]`. In copy B, press `[DEL]`, `[GRP]`, `[3]`, `[ENTER]`. For each copy, capture the response, Group object list, object events, and resolved engine snapshot. Both operations must delete Group 3 and must not create an empty Group. If a derived Group depends on Group 3, both commands must instead reject the deletion with the same dependency reason and leave every Group unchanged.

**Lightning Desk selection with a Group UI target:**

- Add and merge: press `[5] [+] [6] [ENTER]`, press `[REC]`, click Group 3 in the pool, and choose **Merge**.
- Add and merge with no pool click or dialog: select fixtures 5 and 6, then press `[REC] [+] [GRP] [3] [ENTER]`.
- Select the live Group plus additions and overwrite: press `[GRP] [3] [+] [5] [+] [6] [ENTER]`, press `[REC]`, click Group 3, and choose **Overwrite**.
- Remove fixture 2 while retaining the other members' order: press `[GRP] [3] [-] [2] [ENTER]`, press `[REC]`, click Group 3, and choose **Overwrite**.
- Re-add fixture 2 at the end: press `[GRP] [3] [+] [2] [ENTER]`, press `[REC]`, click Group 3, and choose **Overwrite**.

The fully entered command `[REC] [GRP] [3] [ENTER]` overwrites; `[REC] [+] [GRP] [3] [ENTER]` merges; and `[REC] [-] [GRP] [3] [ENTER]` subtracts. None opens the UI confirmation dialog. With an empty selection, the subtract form deletes Group 3 and is equivalent to `[DEL] [GRP] [3] [ENTER]`. Mixed live-reference expressions such as `[GRP] 3 [+] 5 [+] 6`, `[GRP] 3 [-] 2`, and `[GRP] 3 [+] 2` are required command-line behaviors; if the parser rejects one or loses the live Group source, record that exact failure instead of substituting a complete manually rebuilt fixture list.

**Self-recursion watchdog:** The live-reference overwrite deliberately records a selection containing Group 3 back into Group 3. Start a 5,000 ms wall-time watchdog immediately before choosing **Overwrite**. The accepted operation must resolve the current Group members plus fixtures 5 and 6 into a concrete ordered membership and must not persist Group 3 as derived from, nested inside, or otherwise referencing itself. If no successful response or rejected-operation response arrives before the watchdog expires, treat the run as a suspected infinite recursion: preserve the command/event tail and server log, terminate the test's exact application and server processes, fail the scenario, and do not wait for the normal suite timeout or attempt another command against the hung process.

**Assertions:**

- At the removal checkpoint, the API and Group UI report `1, 3, 4, 5, 6`; fixtures 1, 3, 4, 5, and 6 output 128 while fixture 2 outputs 0.
- At the re-addition checkpoint, the API and Group UI report `1, 3, 4, 5, 6, 2`; fixtures 1–6 all output 128.
- Subtraction preserves the relative order of retained members. A later addition appends the previously removed fixture to the end rather than restoring its former position.
- A single expression containing `[-]` followed later by `[+]` is evaluated from left to right and follows the same remove-then-append rule.
- `RECORD + GROUP 3` appends only new selected fixtures; `RECORD - GROUP 3` removes exactly the selected fixtures; empty-selection `RECORD - GROUP 3` and `DELETE GROUP 3` have the same accepted or dependency-rejected result.
- Recording the live `Group 3 + fixtures 5 and 6` selection back into Group 3 completes inside the recursion-watchdog deadline, stores concrete ordered membership, and leaves no direct or indirect self-reference.
- The active programmer contains one group-scoped value, not copied fixture values.
- Art-Net universe 1 and sACN universe 101 contain identical logical slot values.

**Pass condition:** Ordered subtraction immediately removes a member from a live Group value, and adding that member later appends it at the end without disturbing retained or unrelated fixtures.

## DIM-002 — Lightning Desk command reaches real output

**Priority:** P0

**Primary layer:** Paired API/UI E2E

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `dim-002.show`, and use the active copy for this scenario.

**Detailed procedure:**

1. Record the current programmer revision and mark both UDP receivers.
2. For `@api`, send the authenticated command-WebSocket message `programmer.group.set` with `{"group_id":"1","attribute":"intensity","value":0.5}`.
3. For `@ui`, press `[GRP] [1] [AT] [5] [0] [ENTER]` on the visible Lightning Desk keypad. Do not type the command into a hidden test input.
4. Wait until the programmer reports Group 1 intensity `0.5` and a command-applied event newer than the recorded revision exists.
5. Call `POST /api/v1/test/clock/advance` first with `{"millis":2999}` and inspect the returned frame and both newer UDP packets.
6. Call the same endpoint with `{"millis":1}` to reach exactly 3,000 ms and inspect one new frame and packet from each protocol.
7. Call it once more with `{"millis":1}` and prove the completed value remains stable.

**Assertions:** All twelve dimmer slots are 128 in a newer ArtDMX packet and a newer E1.31 packet. The packets use destination universes 1 and 101 respectively.

**Boundary checks:** At virtual 2,999 ms every slot is below 128; at 3,000 ms every slot is exactly 128; advancing further does not change the value.

**Pass condition:** The typed desk path, application fade, engine render, packet encoders, and UDP senders agree at the exact fade boundary.

## CMD-001 — Fixture and Group default modes toggle and scope selections

**Priority:** P0
**Primary layer:** Paired API/UI E2E

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `cmd-001.show`, and use the active copy for this scenario.

**Setup:** Keep Group 3 as fixtures `1, 2, 3, 4` in that order. Set Group 4 to fixtures `9, 10` and Group 5 to fixtures `5, 6, 7, 8`, each in that order. Begin with an empty selection, an empty command line, and Fixture as the default target mode.

**Mode, placeholder, and dereference rule:** A bare number is interpreted using the persistent default target mode. The initial mode is Fixture. Entering only `[GRP] [ENTER]` toggles the default between Group and Fixture without selecting anything. After every toggle, the command line is empty and its visible placeholder identifies the active default as `GROUP` or `FIXTURE`. An explicit `[GRP]` inside a selection applies to the Group term or Group range that follows it; it does not permanently change the default mode and does not leak across `[+]` into a later unprefixed term. Group default mode does not dereference Groups. Only pressing `[GRP]` twice for a particular term dereferences that term. The first press displays `GROUP`; the second press removes that text and replaces it with `DEGRP` rather than appending another `GROUP`.

**Exact UI procedure:** After every completed selection below, inspect the result and then press `[CLR]` exactly once before continuing. That Clear is part of the test and prevents a previous open selection from accumulating into the next case.

1. Confirm the initial command-line placeholder identifies `FIXTURE` and the command line is empty.
2. Press `[GRP] [ENTER]`.
   - **Expect:** No fixture is selected, the persistent default mode becomes Group, and the empty command line's placeholder identifies `GROUP`.
3. Press `[GRP]` once and inspect the command line before pressing anything else.
   - **Expect:** The command line contains exactly `GROUP`.
4. Press `[3] [ENTER]`.
   - **Expect:** Group 3 is selected as a live Group reference. Entering an explicit `GROUP 3` while the persistent default is already Group must not be interpreted as a double-Group press and must not dereference it. After execution, the command clears and the placeholder still identifies `GROUP`.
5. Press `[CLR]`, then press `[GRP]` once.
   - **Expect:** The command line contains exactly `GROUP`.
6. Without entering a number, press `[GRP]` a second consecutive time.
   - **Expect:** The command line changes from `GROUP` to exactly `DEGRP`. It must not display `GROUP GROUP`, and it must not toggle the persistent default mode.
7. Press `[3] [ENTER]`.
   - **Expect:** The current members of Group 3—fixtures `1, 2, 3, 4`—are selected as individual fixture sources. No live Group 3 source remains in the selection. After execution, the persistent default and placeholder remain `GROUP`.
8. Press `[CLR]`, then enter `[GRP] [GRP] [3] [+] [5] [ENTER]`, pausing after each `[GRP]` to assert the same `GROUP` then `DEGRP` text transition.
   - **Expect:** Only Group 3 is dereferenced. The ordered sources are `Fixture 1, Fixture 2, Fixture 3, Fixture 4, Group 5`; Group 5 remains a live reference because the `DEGRP` scope ended before `[+]`. The resolved fixtures are `1, 2, 3, 4, 5, 6, 7, 8`.
9. Press `[CLR]`, then press `[3] [+] [5] [ENTER]`.
   - **Expect:** The command is interpreted as Group 3 plus Group 5. The ordered source list is `Group 3, Group 5`; the resolved selected fixtures are `1, 2, 3, 4, 5, 6, 7, 8`.
10. Press `[CLR]`, then press `[GRP] [ENTER]`.
   - **Expect:** The selection is empty, the persistent default mode becomes Fixture, and the empty command line's placeholder identifies `FIXTURE`.
11. Press `[3] [+] [5] [ENTER]`.
   - **Expect:** Only fixtures 3 and 5 are selected, in that order. Neither term is stored as a Group reference.
12. Press `[CLR]`, then press `[GRP] [3] [+] [5] [ENTER]`.
   - **Expect:** Group 3 is selected by live reference and the unprefixed term after `[+]` uses the persistent Fixture default. The ordered source list is `Group 3, Fixture 5`; the resolved selected fixtures are `1, 2, 3, 4, 5`.
13. Press `[CLR]`, then press `[GRP] [3] [+] [GRP] [5] [ENTER]`.
   - **Expect:** Both terms are explicit live Group references. The ordered source list is `Group 3, Group 5`; the resolved selected fixtures are `1, 2, 3, 4, 5, 6, 7, 8`.
14. Press `[CLR]`, then press `[GRP] [3] [THRU] [5] [ENTER]`.
   - **Expect:** `[THRU]` continues the explicit Group range, selecting Group 3, Group 4, and Group 5 by live reference. The resolved ordered fixture selection is `1, 2, 3, 4, 9, 10, 5, 6, 7, 8`.
15. Press `[CLR]`, then press `[GRP] [3] [THRU] [5] [+] [6] [ENTER]`.
   - **Expect:** The Group range ends before `[+]`. The final unprefixed `6` uses the persistent Fixture default, so the ordered sources are `Group 3, Group 4, Group 5, Fixture 6`. Fixture 6 already occurs through Group 5 and therefore appears only once in the normalized resolved selection.
16. Press `[CLR]` and confirm the placeholder still identifies `FIXTURE`. The explicit `GROUP` and `DEGRP` terms must not have toggled or otherwise changed the persistent default.

**API variant:** Drive the same mode-toggle and command-token operations through the versioned command API. After every operation, assert the same persistent mode, command-line text/placeholder model, ordered source references, normalized targets, and selection revision as the UI variant.

**Assertions:**

- `[GRP] [ENTER]` is a mode toggle, not an empty or invalid Group selection, and each press changes the persistent default exactly once.
- The visible placeholder agrees with the stored default mode after the command line clears.
- Group default mode and an explicit single `[GRP]` term retain live Group references; neither implicitly dereferences a Group.
- On a consecutive double press, the first `[GRP]` displays `GROUP` and the second replaces it with `DEGRP`. `DEGRP <number>` resolves that Group into individual fixture sources.
- `DEGRP` applies only to its own term. A later Group-default or explicit `GROUP` term in the same expression remains live.
- Bare numbers use the current default mode. Explicit `[GRP]` terms override that default only for their own term or range.
- `[+]` terminates an explicit Group term or Group range, so a later unprefixed number returns to the persistent default mode.
- `[GRP] 3 [THRU] 5` expands Group numbers 3, 4, and 5 rather than fixture numbers 3 through 5.
- Source order and live Group references remain visible in programmer state, while overlapping resolved fixtures are deduplicated without changing the source list.

**Pass condition:** An operator can toggle the command line's default address type, distinguish live `GROUP` from scoped `DEGRP`, mix Group and Fixture terms, and enter Group ranges without mode leakage or ambiguous selection state.

## GROUP-003 — Derived group follows source ordering

**Priority:** P1  
**Primary layer:** Rust integration plus one UI E2E

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `group-003.show`, and use the active copy for this scenario.

**Detailed procedure:**

1. Leave empty Group 4 untouched. Press `[GRP] [1] [DIV] [2] [ENTER]` to select positions 1, 3, 5, 7, 9, and 11 from Group 1 while retaining the derived rule.
2. Press `[REC] [GRP] [5] [ENTER]` to record that referenced subset as Group 5. Confirm through the API that Group 5 stores `derived_from` Group 1 with the every-second rule rather than a frozen fixture list.
3. Rebuild Group 1's first edited order by clicking fixtures `12, 1, 2, 4, 5, 6, 7, 8, 9, 10, 11` in that order. Fixture 3 is deliberately omitted.
4. Press `[REC]`, click Group 1, and choose **Overwrite**.
5. Confirm Group 5 recalculates before making the next edit.
6. Rebuild Group 1 again by clicking `12, 1, 2, 8, 4, 5, 6, 7, 9, 10, 11` in that order.
7. Press `[REC]`, click Group 1, and choose **Overwrite**. Confirm Group 5 recalculates again and Group 4 remains empty.

**Assertions:** Group 5 is recalculated from the latest source order after every edit. Its displayed order, API order, and selection order match. Empty group 4 remains unchanged.

**Pass condition:** Derived membership never becomes an accidental frozen copy and always applies its rule to the current ordered source.

## GROUP-004 — Frozen selection does not drift

**Priority:** P1  
**Primary layer:** Playwright E2E

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `group-004.show`, and use the active copy for this scenario.

**Detailed procedure:**

1. Double-press Group 1 in the Groups pool to dereference it into its current individual fixtures. Confirm the source indicator says frozen/static rather than live Group 1.
2. Press `[REC] [GRP] [5] [ENTER]` to store the captured order as Group 5.
3. Rebuild Group 1 by clicking `12, 1, 2, 4, 5, 6, 7, 8, 9, 10, 11` in order, press `[REC]`, click Group 1, and choose **Overwrite**. This adds/reorders fixture 12 and removes fixture 3 from Group 1.
4. Open **Setup → Patch**, press `[SET]` to arm patch editing, and click the **Patch** address value for one fixture still captured by Group 5.
5. In **Set fixture address**, clear the `universe.address` field completely and click **Set**. Confirm the row now says **Unpatched**.
6. Reopen Groups and confirm Group 5 kept its original ordered capture and does not report the unpatched member as missing.
7. Program Group 5 to a visible intensity, store that look in a cue or preset, and confirm the unpatched member remains represented in the fixture sheet, stage view, and stored object data while producing no DMX output.

**Assertions:** Group 5 retains its captured order. The unpatched member remains a valid fixture reference and is not silently discarded, replaced, or reported as missing. Group programming and stored looks include the unpatched fixture. Only DMX output changes: the fixture produces no DMX while unpatched, then resumes output if it is patched again.

**Pass condition:** A frozen group remains reproducible across source edits and patch edits. Unpatching a fixture affects physical DMX output only, not show programming, fixture-sheet/stage visibility, or group membership.

## GROUP-005 — Empty and invalid references fail safely

**Priority:** P1  
**Primary layer:** Rust/API integration

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `group-005.show`, and use the active copy for this scenario.

**Detailed cases:**

1. From a fresh working copy, confirm Group 4 is not present as a stored Group object after deletion or before deliberate creation. Enter a Group range that spans the missing slot, such as `[GRP] [1] [THRU] [5] [ENTER]`, and confirm the missing Group 4 is skipped like a nonexistent fixture number; the range selects only stored groups in order and does not create Group 4.
2. With an empty selection, press `[REC]`, then click Group 4's empty pool cell. Because no Group object exists there yet, no Merge/Overwrite dialog should open; the click stores Group 4 directly as an empty Group object. If the `@ui` surface cannot record an empty selection this way, mark that UI case blocked on empty-cell Group storage; the `@api` variant must create the same revision-checked empty Group object directly.
3. Click stored empty Group 4, set intensity to 50%, and confirm the programmer contains a live Group 4 value even though no fixture output changes.
4. From independent copies with stored empty Group 4 and fixture 1 selected, verify both record modes produce the same result: `RECORD + GROUP 4` and `RECORD GROUP 4` each store fixture 1 as Group 4's only member, and after advancing virtual time by 3,000 ms only fixture 1 receives the live Group 4 value.
5. In the UI variant, click fixture 1, press `[REC]`, and click stored empty Group 4. Because Group 4 exists but has no members, no Merge/Overwrite dialog should open; the click stores fixture 1 directly, equivalent to either Merge or Overwrite.
6. Delete Group 4. Confirm a direct `GROUP 4` selection or `RECORD + GROUP 4` merge rejects because the group no longer exists, while a later Group range spanning 4 skips it without creating it.
7. **Harness only:** submit a derived Group object whose `source_group_id` does not exist and assert rejection without a revision change.
8. **Harness only:** submit one direct self-cycle and one two-Group cycle using revision-checked Group object updates. Assert each write is rejected atomically.
9. **Harness only:** create a valid source and derived dependant, then issue the authenticated DELETE for the source Group. Assert the documented dependant-protection error and unchanged bytes/revisions.

**Assertions:** Every rejected operation returns the documented error and preserves the previous object revision and bytes. Stored empty-group mutations succeed, generate one revision, and affect output only after a real member is added. Unstored, never-stored, or deleted Group IDs are not empty groups; ranges skip them and direct operations that require a live Group reject them.

**Pass condition:** Stored empty groups are valid and deterministic. Missing or deleted groups remain absent unless explicitly recorded, group ranges skip absent IDs, and missing or cyclic references return explicit errors without partial mutation.

## PROG-001 — Selection persists through value entry until replaced or cleared

**Priority:** P0

**Primary layer:** Paired API/UI E2E

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `prog-001.show`, and use the active copy for this scenario.

**Selection rule:** A fixture or group selection remains current after a programmer value, encoder move, or preset recall applies to it. The operator can keep working on the same selection with another value command. The next fixture or group selection that does not begin with `+` replaces the previous selection and starts a new current selection; a leading `+` continues the previous selection instead. Pressing `CLR` once clears the current selection without clearing programmed values.

**Actions and checkpoints:**

1. In the Stage view, click fixture 1 and then click fixture 2 without changing any value. Use ordinary clicks; do not hold Command, Control, or Shift.
   - **Expect:** Fixtures 1 and 2 are both selected.
2. Press on empty Stage space, drag a marquee around fixtures 3 and 4, and release. Do not hold a modifier.
   - **Expect:** Fixtures 3 and 4 are added; the selection is fixtures 1–4, not only the marquee result.
3. Open the Fixture Sheet and click the row for fixture 5. Open Groups and click Group 2 once.
   - **Expect:** Fixture 5 and every member of group 2 are added to the existing selection. Switching surfaces does not start a new selection, and repeated fixture membership is not duplicated.
4. Click Group 3 and then Group 1 without applying a value between them.
   - **Expect:** Both group selections accumulate. The resolved target set is the ordered union of all fixture and group selections made since the last boundary, equivalent to joining those selections with `+` on the command line.
5. Touch the Intensity parameter control, enter `50`, and confirm it.
   - **Expect:** The value applies to the complete accumulated selection, and that same selection remains visible and current.
6. Without selecting anything else, touch the Intensity parameter control again, enter `25`, and confirm it.
   - **Expect:** The same accumulated selection receives the new value. No target is removed merely because a previous value was applied.
7. Return to Stage and click fixture 21 once without a modifier.
   - **Expect:** Fixture 21 replaces the previous selection and becomes the only current selection. The values already programmed for the previous selection remain unchanged.
8. Click fixture 22 once, open Presets, select a compatible preset family, and click a populated preset cell once.
   - **Expect:** Fixture 22 is first added to fixture 21. The preset applies to both, and fixtures 21 and 22 remain the current selection.
9. Enter `[+] [2] [3] [ENTER]`, then recall another compatible preset or set a supported attribute value.
   - **Expect:** The leading `+` continues the current selection after the prior preset recall, producing fixtures `21, 22, 23`; the new preset or value applies to all three current targets.
10. Open Groups, click Group 3 once, then touch Intensity, enter `25`, and confirm it.
   - **Expect:** Group 3 replaces fixtures 21, 22, and 23 before the value is applied, and Group 3 remains the current selection after the value edit.
11. Click fixture 6, press `[CLR]` exactly once, click fixture 7, open Groups, and click Group 3. Do not press another key or use a modifier between those selection actions.
   - **Expect:** Fixture 6 starts a new selection because it did not begin with `+`. The first `CLR` clears that selection without clearing its programmer values. Fixture 7 and group 3 then accumulate into a fresh current selection.

**Assertions:**

- After every selection action, the API programmer selection and all visible selection indicators report the same normalized ordered targets and source references.
- Stage clicks, Stage marquee selection, Fixture Sheet rows, Group UI entries, and mixed fixture/group operations all use the same current-selection rule; none requires a modifier key to accumulate before a value is applied.
- Selecting through another surface alone never applies a replacement boundary and never discards targets selected through an earlier surface.
- A value entry, encoder change, or preset recall applies to the whole current selection and leaves that selection current. The next non-plus selection action replaces it; a leading `+` continues it.
- The first `CLR` explicitly clears the current selection while preserving programmer values, consistent with `PROG-004`.
- Repeated or overlapping fixture targets occur only once in the resolved selection. Group source references remain identifiable rather than being silently rewritten as unrelated manual fixture selections.

**Pass condition:** Consecutive fixture and group selections accumulate across every operator surface, value edits leave the selection current, a leading `+` can continue that current selection, and the next non-plus selection or first `CLR` starts cleanly without altering values already applied to the previous targets.

## PROG-002 — Values spread across ordered selections

**Priority:** P0
**Primary layer:** Paired API/UI E2E plus engine interpolation checks

**Starting show:** For every numbered case, load canonical `compact-rig.show`, immediately Save As a fresh `prog-002-<case>.show`, and use only that active copy. Do not carry programmer values from one case into another.

**Setup:** Set Group 1 to exactly ten fixtures in the order `1, 2, 3, 4, 5, 6, 7, 8, 9, 10`. Start in Fixture default mode with an empty command line, selection, and programmer. Every command below is completed by pressing `[ENTER]`.

**Spread rule:** Values separated by `[THRU]` are control points spread over the ordered selection. A one-value command applies that value to every selected fixture. A two-value command places the first and last selected target positions at the stated endpoints and linearly interpolates equally spaced values for every target position between them. Reversing the endpoints reverses the resulting values without changing selection order.

Live Group references and dereferenced Groups must not collapse into the same storage shape:

- `GROUP <n> AT <spread>` stores a group-relative spread against Group `<n>`, not fixture-scoped values. The spread is evaluated over Group `<n>`'s current ordered membership whenever it is rendered, stored in a Cue/Preset, or recalled.
- If Group `<n>` is later overwritten with members added, removed, or reordered, the existing group-relative spread immediately recalculates across the new ordered membership. New members receive their interpolated position in the spread; removed members no longer receive the group-relative value.
- `GROUP GROUP <n> AT <spread>` / `DEGRP <n> AT <spread>` first dereferences Group `<n>` into its current fixture order and then stores fixture-scoped values. Later edits to Group `<n>` must not add values to new members, remove values from former members, or recalculate existing fixture values.
- Storing a live Group spread into a Cue or Preset must preserve the group-relative spread. Storing a dereferenced spread must store fixture addresses and their concrete values.

**Exact dimmer cases:**

1. **Uniform zero:** Press `[GRP] [1] [AT] [0] [ENTER]`.
   - **Expect:** Fixtures 1–10 each have intensity `0%` and output DMX value `0`.
2. **Ascending spread:** Press `[GRP] [1] [AT] [0] [THRU] [1] [0] [0] [ENTER]`.
   - **Expect:** Fixtures 1–10 receive normalized intensity values `0/9, 1/9, 2/9, 3/9, 4/9, 5/9, 6/9, 7/9, 8/9, 9/9` in Group order. The corresponding 8-bit output values are `0, 28, 57, 85, 113, 142, 170, 198, 227, 255` using the production encoder's rounding.
3. **Descending spread:** Press `[GRP] [1] [AT] [1] [0] [0] [THRU] [0] [ENTER]`.
   - **Expect:** Fixtures 1–10 receive the exact reverse of the ascending normalized and DMX sequences: `255, 227, 198, 170, 142, 113, 85, 57, 28, 0` on output.
4. **Multiple control points:** Press `[GRP] [1] [AT] [1] [0] [0] [THRU] [0] [THRU] [1] [0] [0] [ENTER]`.
   - **Expect now:** Fixture 1 and fixture 10 are at `100%`; the values are symmetric; values decrease monotonically toward the center and increase monotonically away from it; the absolute step size is constant on each side; and the two center fixtures have the same value. Do not assert whether fixtures 5 and 6 are `0%` or a nonzero interpolated value until the even-selection rule is decided in the development open questions.

**Reference and dereference cases:**

5. **Live Group spread follows membership:** From a fresh copy, press `[GRP] [1] [AT] [0] [THRU] [1] [0] [0] [ENTER]`. Confirm the programmer stores one Group 1 spread value model, not ten fixture values. Overwrite Group 1 to `12, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10` and advance to the fade boundary. The spread now runs across eleven members in that exact order; fixture 12 receives the first endpoint, fixture 10 receives the last endpoint, and the original fixtures shift to their new interpolated positions.
6. **Live Group spread stores as a reference:** Store the live Group 1 ascending spread into a Cue and into an Intensity Preset. Inspect the stored objects and assert they contain a Group 1 relative spread, not expanded fixture values. After editing Group 1's membership, recall the Cue/Preset and confirm the same recalculated eleven-member output as the live programmer case.
7. **Dereferenced Group spread freezes fixture values:** From a fresh copy, press `[GRP] [GRP] [1] [AT] [0] [THRU] [1] [0] [0] [ENTER]`. Confirm the programmer stores fixture-scoped values for the ten current Group 1 members. Overwrite Group 1 to add fixture 12 at the start and reorder the remaining members. Fixture 12 receives no value from the dereferenced spread; the original ten fixture values remain attached to their fixture IDs with the values calculated at the time of dereference.

For every case, assert the programmer's normalized values and address shape before advancing virtual time: live Group cases expose group-relative spread data, while dereferenced cases expose fixture-scoped values. Then advance exactly to the programmer-fade boundary and assert Fixture Sheet values plus matching logical, Art-Net, and sACN output. The interpolation order is the ordered live Group membership for referenced Groups, and the captured fixture order for dereferenced Groups. It is never fixture-number sorting performed after selection resolution.

**Deferred complex-value cases — intentionally empty:**

8. **Color spread:** Reserve coverage for spreading multiple color values across an ordered selection. The command syntax, color space, hue-path behavior, and interpolation rules are not specified. Do not invent keystrokes or expected intermediate colors; leave the action and value assertions empty until those decisions are documented.
9. **Position spread:** Reserve coverage for spreading multiple position values across an ordered selection. The command syntax, pan/tilt path, wrap behavior, calibration space, and interpolation rules are not specified. Do not invent keystrokes or expected intermediate positions; leave the action and value assertions empty until those decisions are documented.

**Assertions:**

- The number of generated values always equals the number of unique ordered targets.
- The first and last targets receive the first and last control-point values exactly.
- A two-point spread has equal normalized intervals, including when the direction is reversed.
- Live Group spreads are stored and recalled as group-relative spread data and recalculate when the Group's ordered membership changes.
- Dereferenced Group spreads are stored and recalled as fixture-scoped values and do not recalculate when the source Group changes.
- Output quantization occurs only after interpolation; intermediate normalized values are not repeatedly rounded while the spread is built.
- Multiple `[THRU]` tokens are parsed as one value spread with multiple control points, not as a selection range or invalid trailing tokens.

**Pass condition:** Uniform, ascending, descending, and multi-control-point intensity commands distribute deterministic values over ordered targets. Live Group spreads remain group-relative through programmer, Cue, and Preset storage; dereferenced Group spreads become concrete fixture values. Unresolved complex-value syntax remains explicitly unimplemented.

## PROG-003 — Programmer intensity override uses LTP within one programmer

**Priority:** P0  
**Primary layer:** Playwright E2E

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `prog-003.show`, and use the active copy for this scenario.

**Detailed procedure:**

1. Open Groups, click Group 1 once, touch Intensity, enter `50`, and confirm it. Confirm the programmer contains one live Group 1 intensity value.
2. Click fixture 1 once. Because this new selection does not begin with `+`, it replaces the current Group selection.
3. Touch Intensity, enter `75`, and confirm it. Confirm the programmer now contains one Group 1 value and one fixture 1 value. Advance virtual time by exactly 3,000 ms and emit one frame: fixture 1 outputs 191 while fixtures 2–12 output 128.
4. Release only fixture 1's fixture-scoped intensity value through the parameter/value release control.
5. **UI capability required:** if no per-attribute release control is present, stop the `@ui` case at step 4; the API variant removes only fixture 1's intensity contribution.
6. Emit a 0 ms frame and confirm fixture 1 falls back to the still-active Group 1 value of 128.
7. From a fresh copy, repeat steps 1 and 2, but set fixture 1 to `25`. Advance to the fade boundary and confirm fixture 1 outputs 64 while fixtures 2–12 remain at 128. This proves the fixture-scoped programmer value wins by recency/specificity even when it is lower than the live Group value.
8. Without releasing fixture 1, enter `[GRP] [1] [AT] [5] [0] [ENTER]` again. Advance to the fade boundary and confirm fixture 1 returns to 128 with the rest of Group 1, because the newer Group 1 programmer value supersedes the older fixture-scoped value inside the same programmer.
9. Release the Group 1 intensity value while leaving the fixture 1 value present. Emit a 0 ms frame and confirm fixture 1 returns to 64 while fixtures 2–12 fall to 0. Then release fixture 1 and confirm all fixtures return to 0.

**Assertions:** Within one programmer, live Group intensity and fixture-scoped intensity resolve by LTP semantics, not HTP. A later lower fixture value can pull a member below its Group value; a later Group value can pull that fixture back up; releasing one source falls back to the next still-active source without rewriting the other value.

**Pass condition:** Programmer-layer intensity behaves as LTP across fixture-scoped and group-scoped programmer values. HTP is reserved for cue/playback or cross-source merge coverage, not this single-programmer override case.

## PROG-004 — Two-stage clear has distinct effects

**Priority:** P0  
**Primary layer:** Playwright E2E

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `prog-004.show`, and use the active copy for this scenario.

**Setup procedure:**

1. Start with an empty selection and empty programmer. Confirm the Clear button is dark.
2. Click Group 1 once. Confirm Group 1 is the current selection and the Clear button is lit.
3. Press `[CLR]` once. Confirm the selection is empty, the programmer remains empty, and the Clear button is dark again.

**Actions and checkpoints:**

1. Press `[1] [+] [2] [AT] [7] [5] [ENTER]`. Confirm fixtures 1 and 2 output 75%, remain the current selection, and the Clear button is lit.
2. Without selecting anything else, press `[AT] [5] [0] [ENTER]`. Confirm the still-current fixtures 1 and 2 now output 50%.
3. Press `[CLR]` once. Read programmer state and emit a 0 ms frame: selection must be empty while the fixture 1 and 2 programmer values and output remain. The Clear button must blink because there is no current selection but the programmer still contains active values.
4. Press `[CLR]` a second time. Read programmer state and emit another 0 ms frame: the programmer values clear, fixtures 1 and 2 return to 0, and the Clear button is dark.
5. From a fresh copy, press `[1] [+] [2] [AT] [7] [5] [ENTER]`, then press `[3] [AT] [8] [0] [ENTER]`. Confirm the second command starts a new selection, so fixtures 1 and 2 keep 75% while fixture 3 outputs 80%.
6. From another fresh copy, press `[1] [+] [2] [AT] [7] [5] [ENTER]`, then press `[+] [3] [AT]` and apply a compatible populated preset or supported attribute value. Confirm the leading `+` continues the previous selection, so the new value applies to fixtures 1, 2, and 3 while the prior intensity on fixtures 1 and 2 remains stored unless the new value explicitly changes intensity.

**Assertions:** After every press, read selection, programmer values, Clear button state, audit tail, and a newly emitted frame. First-stage Clear only clears selection; second-stage Clear only clears the remaining programmer values. Value commands do not deselect the targets they just modified.

**Pass condition:** Clear is a two-stage operation with distinct UI indication: dark with no selection or programmer values, lit while a selection is current, and blinking when only programmer values remain. Selection replacement and leading-plus continuation behave identically through the command line and clickable surfaces.

## Follow-ups

| Scenario | Next tests after the primary case | First failure checks |
| --- | --- | --- |
| DIM-001 | Subtract and re-add ranges, subtract absent fixtures, repeat while a cue holds a fixture-scoped value, and repeat after save/reopen. | Compare expression order, stored group order, live programmer reference, resolved values, then packet slots. |
| DIM-002 | Test 0%, 25%, 75%, and 100%; repeat through typed input rather than keypad clicks. | Inspect command text, command-applied audit payload, programmer timestamp, and packet mark. |
| CMD-001 | Repeat from Group default mode with explicit Fixture syntax once that syntax is defined; cover nonexistent and empty Groups. | Inspect persistent default mode, placeholder model, token scope, source references, and normalized targets after every Enter. |
| GROUP-003 | Test odd/even/every-N rules and insertion at every position. | Compare source order and rule output before looking at rendering. |
| GROUP-004 | Refresh the frozen group explicitly and verify its capture revision changes once. | Inspect captured IDs/revision and missing-fixture diagnostics. |
| GROUP-005 | Add deeper chains up to the supported limit and concurrent stale-revision writes. | Assert the failed mutation left the original object and revision unchanged. |
| PROG-001 | Repeat with drag/range selection where supported, overlapping groups, reversed surface order, preset families, and all encoder-backed attributes. | Compare the current selection state, ordered sources, resolved targets, leading-plus continuation, and last programmer mutation after each action. |
| PROG-002 | Decide even-count multi-point interpolation, then add odd/even target counts, unequal segment lengths, color values, position values, and additional Group membership edit shapes. | Inspect ordered targets, live-versus-dereferenced address shape, normalized pre-quantization values, control-point placement, stored Cue/Preset data, and encoded output. |
| PROG-003 | Add same-level repeated edits, release sources in every order, and then combine with a second user/playback in MERGE coverage. | Inspect programmer contribution timestamps/scope, LTP source selection, fallback behavior, and encoded output. |
| PROG-004 | Repeat in preload, blind, and preview contexts. | Capture selection, values, Clear button indication, context, audit, and DMX after each individual `CLR`. |
