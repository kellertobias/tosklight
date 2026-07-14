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

1. Select group 3 in the Group UI and set it to 50%.
2. Advance virtual time by 3,000 ms and emit one frame.
3. Add fixtures 5 and 6 to the end of group 3.
4. Remove fixture 2 and move fixture 6 before fixture 1.
5. Emit another frame without rewriting the programmer value.

**Assertions:**

- The API and Group UI report the order `6, 1, 3, 4, 5`.
- Fixtures 1, 3, 4, 5, and 6 output 128; fixture 2 outputs 0.
- The active programmer contains one group-scoped value, not copied fixture values.
- Art-Net universe 1 and sACN universe 101 contain identical logical slot values.

**Pass condition:** Ordered membership changes immediately affect a live group reference without disturbing removed or unrelated fixtures.

## DIM-002 — Lightning Desk command reaches real output

**Priority:** P0

**Primary layer:** Paired API/UI E2E

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `dim-002.show`, and use the active copy for this scenario.

**API action:** Send `programmer.group.set` for group 1, intensity, value 0.5 through the authenticated command WebSocket.

**UI action:** Enter `GROUP 1 AT 50` using actual Lightning Desk keypad buttons and wait for the command-applied event.

**Shared completion:** Advance exactly 3,000 ms and request one frame.

**Assertions:** All twelve dimmer slots are 128 in a newer ArtDMX packet and a newer E1.31 packet. The packets use destination universes 1 and 101 respectively.

**Boundary checks:** At virtual 2,999 ms every slot is below 128; at 3,000 ms every slot is exactly 128; advancing further does not change the value.

**Pass condition:** The typed desk path, application fade, engine render, packet encoders, and UDP senders agree at the exact fade boundary.

## GROUP-003 — Derived group follows source ordering

**Priority:** P1  
**Primary layer:** Rust integration plus one UI E2E

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `group-003.show`, and use the active copy for this scenario.

**Setup:** Leave empty group 4 `Center Spot` unchanged. Create group 5 as every second member of group 1, starting with the first member.

**Actions:** Insert fixture 12 at the start of group 1, remove fixture 3, and reorder fixture 8 before fixture 4.

**Assertions:** Group 5 is recalculated from the latest source order after every edit. Its displayed order, API order, and selection order match. Empty group 4 remains unchanged.

**Pass condition:** Derived membership never becomes an accidental frozen copy and always applies its rule to the current ordered source.

## GROUP-004 — Frozen selection does not drift

**Priority:** P1  
**Primary layer:** Playwright E2E

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `group-004.show`, and use the active copy for this scenario.

**Setup:** Capture group 1 as frozen group 5.

**Actions:** Add, remove, and reorder members in group 1; then unpatch one fixture captured by group 5.

**Assertions:** Group 5 retains its captured order. The unpatched member is represented as missing rather than silently discarded or replaced.

**Pass condition:** A frozen group remains reproducible and reports stale members explicitly.

## GROUP-005 — Empty and invalid references fail safely

**Priority:** P1  
**Primary layer:** Rust/API integration

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `group-005.show`, and use the active copy for this scenario.

**Cases:**

- Program empty group 4 `Center Spot`, then add a member and verify the live value becomes effective.
- Remove every member from an active group and verify unrelated output remains unchanged.
- Reference a nonexistent source group.
- Create a direct or indirect derived-group cycle.
- Delete a source group that still has derived dependants.

**Assertions:** Every rejected operation returns the documented error and preserves the previous object revision and bytes. Empty-group mutations succeed, generate one revision, and affect output only after a real member is added.

**Pass condition:** Empty groups are valid and deterministic; missing or cyclic references return explicit errors without partial mutation.

## PROG-001 — Selection accumulates until it is used or cleared

**Priority:** P0

**Primary layer:** Paired API/UI E2E

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `prog-001.show`, and use the active copy for this scenario.

**Selection rule:** A fixture or group selection remains open and additive until the operator either changes a programmer value for that selection or presses `CLR` once. While it is open, every later fixture or group selection is added as if the operator had entered the selections with `+` between them. Merely changing the selection does not close it. After the selection has been used to add or change a value, apply an encoder value, or recall a preset, the next fixture or group selection replaces the previous selection and starts a new open selection.

**Actions and checkpoints:**

1. In the Stage view, click fixture 1 and then fixture 2 without changing any value.
   - **Expect:** Fixtures 1 and 2 are both selected.
2. Use a Stage marquee around fixtures 3 and 4.
   - **Expect:** Fixtures 3 and 4 are added; the selection is fixtures 1–4, not only the marquee result.
3. In the Fixture Sheet, select fixture 5 and then select group 2 in the Groups view.
   - **Expect:** Fixture 5 and every member of group 2 are added to the existing selection. Switching surfaces does not start a new selection, and repeated fixture membership is not duplicated.
4. Select group 3 and then group 1 without applying a value between them.
   - **Expect:** Both group selections accumulate. The resolved target set is the ordered union of all fixture and group selections made since the last boundary, equivalent to joining those selections with `+` on the command line.
5. Set intensity with an encoder.
   - **Expect:** The value applies to the complete accumulated selection and marks that selection as used.
6. Click fixture 21.
   - **Expect:** The previous selection is replaced by fixture 21; fixture 21 begins a new open selection. The values already programmed for the previous selection remain unchanged.
7. Click fixture 22, then recall a compatible preset for the two selected fixtures.
   - **Expect:** Fixture 22 is first added to fixture 21. The preset applies to both and marks that selection as used.
8. Select group 3, then use the command/value controls to add or change a programmer value.
   - **Expect:** Group 3 replaces fixtures 21 and 22 before the value is applied, and the value edit marks group 3's selection as used.
9. Select fixture 6, press `CLR` once, and then select fixture 7 followed by group 3.
   - **Expect:** Fixture 6 starts a new selection because the preceding group selection was used. The first `CLR` clears that selection without clearing its programmer values. Fixture 7 and group 3 then accumulate into a fresh open selection.

**Assertions:**

- After every selection action, the API programmer selection and all visible selection indicators report the same normalized ordered targets and source references.
- Stage clicks, Stage marquee selection, Fixture Sheet rows, Group UI entries, and mixed fixture/group operations all use the same additive-until-used rule; none requires a modifier key to accumulate.
- Selecting through another surface alone never marks the selection as used and never discards targets selected through an earlier surface.
- A value entry, encoder change, or preset recall marks the current selection as used only after applying to the whole current selection. The next selection action replaces it; the edit itself does not clear it.
- The first `CLR` explicitly closes and clears the current selection while preserving programmer values, consistent with `PROG-004`.
- Repeated or overlapping fixture targets occur only once in the resolved selection. Group source references remain identifiable rather than being silently rewritten as unrelated manual fixture selections.

**Pass condition:** Consecutive fixture and group selections accumulate across every operator surface until the selection is programmed or explicitly cleared; the next selection after either boundary starts cleanly without altering values already applied to the previous targets.

## PROG-003 — Fixture intensity override uses HTP

**Priority:** P0  
**Primary layer:** Playwright E2E

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `prog-003.show`, and use the active copy for this scenario.

**Actions:** Put group 1 at 50%, then put fixture 1 at 75%. Advance to the fade boundary and emit one frame.

**Assertions:** Fixture 1 outputs 191 while fixtures 2–12 output 128. Clearing the fixture-scoped value restores fixture 1 to 128 without rewriting the group value.

**Pass condition:** Intensity contributions merge by HTP and release to the next valid source.

## PROG-004 — Three-stage clear has distinct effects

**Priority:** P0  
**Primary layer:** Playwright E2E

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `prog-004.show`, and use the active copy for this scenario.

**Setup:** Select group 1, apply intensity, and prepare a pending preload value.

**Actions and checkpoints:**

1. First `CLR`: selection clears; values and output remain.
2. Second `CLR`: normal programmer values clear; active playback and preload state remain.
3. Third `CLR`: the complete programmer/session context clears according to the command-line contract.

**Assertions:** After every press, read selection, normal and preload values, active context, audit tail, and a newly emitted frame. Only the fields assigned to that clear stage may change.

**Pass condition:** Each clear stage changes only its documented scope and produces matching UI, API, audit, and DMX state.

## Follow-ups

| Scenario | Next tests after the primary case | First failure checks |
| --- | --- | --- |
| DIM-001 | Repeat while a cue holds a fixture-scoped value; repeat after save/reopen. | Compare stored group order, live programmer reference, resolved values, then packet slots. |
| DIM-002 | Test 0%, 25%, 75%, and 100%; repeat through typed input rather than keypad clicks. | Inspect command text, command-applied audit payload, programmer timestamp, and packet mark. |
| GROUP-003 | Test odd/even/every-N rules and insertion at every position. | Compare source order and rule output before looking at rendering. |
| GROUP-004 | Refresh the frozen group explicitly and verify its capture revision changes once. | Inspect captured IDs/revision and missing-fixture diagnostics. |
| GROUP-005 | Add deeper chains up to the supported limit and concurrent stale-revision writes. | Assert the failed mutation left the original object and revision unchanged. |
| PROG-001 | Repeat with drag/range selection where supported, overlapping groups, reversed surface order, preset families, and all encoder-backed attributes. | Compare the selection's open/used state, ordered sources, resolved targets, and last programmer mutation after each action. |
| PROG-003 | Add a second user and a playback contribution; release sources in every order. | Inspect each contribution and HTP resolution before packet conversion. |
| PROG-004 | Repeat in preload, blind, and preview contexts. | Capture selection, values, context, audit, and DMX after each individual `CLR`. |
