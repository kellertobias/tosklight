# Foundational Dimmers and Groups

These scenarios use Bench A and form the fastest confidence suite. Unless noted otherwise, assertions inspect application state and both real UDP outputs.

## How to run this file

Create Bench A through the Playwright fixture before every scenario. Open the production UI only for scenarios marked Playwright; API/Rust cases should avoid browser startup. Before each action, capture the current revision and both UDP receiver marks. After the action, wait for the programmer or object revision, advance to the stated virtual timestamp, and decode the first newer Art-Net and sACN packets.

## DIM-001 — Ordered group edits remain live

**Priority:** P0  
**Primary layer:** Playwright E2E

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
**Primary layer:** Playwright E2E

**Actions:** Enter `GROUP 1 AT 50` using actual Lightning Desk keypad buttons, wait for the command-applied event, advance exactly 3,000 ms, and request one frame.

**Assertions:** All twelve dimmer slots are 128 in a newer ArtDMX packet and a newer E1.31 packet. The packets use destination universes 1 and 101 respectively.

**Boundary checks:** At virtual 2,999 ms every slot is below 128; at 3,000 ms every slot is exactly 128; advancing further does not change the value.

**Pass condition:** The typed desk path, application fade, engine render, packet encoders, and UDP senders agree at the exact fade boundary.

## GROUP-003 — Derived group follows source ordering

**Priority:** P1  
**Primary layer:** Rust integration plus one UI E2E

**Setup:** Create group 4 as every second member of group 1, starting with the first member.

**Actions:** Insert fixture 12 at the start of group 1, remove fixture 3, and reorder fixture 8 before fixture 4.

**Assertions:** Group 4 is recalculated from the latest source order after every edit. Its displayed order, API order, and selection order match.

**Pass condition:** Derived membership never becomes an accidental frozen copy and always applies its rule to the current ordered source.

## GROUP-004 — Frozen selection does not drift

**Priority:** P1  
**Primary layer:** Playwright E2E

**Setup:** Capture group 1 as frozen group 5.

**Actions:** Add, remove, and reorder members in group 1; then unpatch one fixture captured by group 5.

**Assertions:** Group 5 retains its captured order. The unpatched member is represented as missing rather than silently discarded or replaced.

**Pass condition:** A frozen group remains reproducible and reports stale members explicitly.

## GROUP-005 — Empty and invalid references fail safely

**Priority:** P1  
**Primary layer:** Rust/API integration

**Cases:**

- Program an empty group, then add a member and verify the live value becomes effective.
- Remove every member from an active group and verify unrelated output remains unchanged.
- Reference a nonexistent source group.
- Create a direct or indirect derived-group cycle.
- Delete a source group that still has derived dependants.

**Assertions:** Every rejected operation returns the documented error and preserves the previous object revision and bytes. Empty-group mutations succeed, generate one revision, and affect output only after a real member is added.

**Pass condition:** Empty groups are valid and deterministic; missing or cyclic references return explicit errors without partial mutation.

## PROG-003 — Fixture intensity override uses HTP

**Priority:** P0  
**Primary layer:** Playwright E2E

**Actions:** Put group 1 at 50%, then put fixture 1 at 75%. Advance to the fade boundary and emit one frame.

**Assertions:** Fixture 1 outputs 191 while fixtures 2–12 output 128. Clearing the fixture-scoped value restores fixture 1 to 128 without rewriting the group value.

**Pass condition:** Intensity contributions merge by HTP and release to the next valid source.

## PROG-004 — Three-stage clear has distinct effects

**Priority:** P0  
**Primary layer:** Playwright E2E

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
| PROG-003 | Add a second user and a playback contribution; release sources in every order. | Inspect each contribution and HTP resolution before packet conversion. |
| PROG-004 | Repeat in preload, blind, and preview contexts. | Capture selection, values, context, audit, and DMX after each individual `CLR`. |
