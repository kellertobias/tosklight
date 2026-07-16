# Cues, Tracking, and Arbitration

Use `compact-rig.show` for the twelve-dimmer tracking cases and `default-stage.show` for the color, position, and arbitration cases. Each scenario's **Starting show** line is authoritative.

## How to run this file

Before every scenario, load its named canonical show, immediately use Save As with the filename stated by that scenario, and use only the active working copy. For the simple twelve-dimmer cases, overwrite Groups 1–3 so Group 1 contains fixtures 1–4, Group 2 contains fixtures 5–8, and Group 3 contains fixtures 9–12, all in ascending order. For the `default-stage.show` cases, create Group 1 as fixtures 1–6, Group 2 as fixtures 101–108, and Group 3 as fixtures 201–205. Build each Cuelist through the named surface, then press `[CLR]` once for selection and again for normal programmer values, and use **OFF** before verification. Capture expected tracked state from stored Cue data, not current output. For each GO, jump, pause, release, Flash, or Temp action, wait for the playback revision, advance to exact virtual checkpoints, and compare playback state, resolved attributes, and received DMX.

## CUE-001 — Record and replay a tracked cue sequence

**Priority:** P0  
**Primary layer:** Playwright E2E

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `cue-001.show`, and use the active copy for this scenario.

**Detailed procedure:**

1. Overwrite Group 1 with fixtures 1–4, Group 2 with fixtures 5–8, and Group 3 with fixtures 9–12. Confirm that no programmer or playback is active and page 1 playback 1 is empty.
2. Select live Group 1, set Intensity to 100%, and confirm. Press `[REC]`, then touch any button belonging to page 1 playback 1.
   - **Expect:** Because both the playback and Cuelist slot 1 are empty, recording creates Cuelist 1 with Cue 1, assigns it to page 1 playback 1, turns the playback on, and jumps it to the Cue just recorded. Cue 1 contains only Group 1 Intensity at 100%.
   - Repeat from independent fresh copies with each physical or simulated button belonging to playback 1 as the Record target. Every button must identify the same playback; its normal GO, GO minus, Flash, Toggle, On, or Off action must not fire while Record is armed.
3. Press `[CLR]` twice, select live Group 2, set Intensity to 100%, press `[REC]`, and touch a button on the already assigned playback 1.
   - **Expect:** Cue 2 is appended to the existing Cuelist and becomes current immediately. Cue 2 stores only the Group 2 change; Group 1 reaches 100% through tracking, not because Group 1 was redundantly rerecorded.
4. Press `[CLR]` twice and turn playback 1 **OFF**. Turn it on again with its configured **ON** or **TOGGLE** action without pressing GO twice.
   - **Expect:** Playback 1 starts at numbered Cue 1. Only fixtures 1–4 are at 100%; fixtures 5–12 are at 0%.
5. Press **GO** once.
   - **Expect:** Numbered Cue 2 becomes current. Fixtures 1–4 remain at 100% by tracking and fixtures 5–8 turn on at 100%. Fixtures 9–12 remain at 0%.
6. Turn playback 1 **OFF**, press `[CLR]` twice, select live Group 3, and set Intensity to 100%.
7. Record the inserted Cue with `[REC] [SET] [1] [CUE] [1] [ . ] [5] [ENTER]`. Before Enter, confirm the command line reads exactly `RECORD SET 1 CUE 1.5`.
   - **Address check:** This command is correct because it explicitly targets Cue 1.5 in Cuelist 1. The equivalent address through page 1 playback 1 is `[REC] [SET] [1] [ . ] [1] [CUE] [1] [ . ] [5] [ENTER]`, displayed as `RECORD SET 1.1 CUE 1.5`. Run both address forms in independent copies and assert identical stored Cuelist data.
   - **Expect:** The ordered Cue numbers are now `1`, `1.5`, and `2`. Cue 1.5 stores only Group 3 Intensity at 100%; inserting it must not rewrite the deltas already stored in Cues 1 or 2.
8. Press `[CLR]` twice, turn playback 1 **OFF**, and turn it on again.
   - **Expect:** Numbered Cue 1 becomes current and only Group 1 is on.
9. Press **GO** once.
   - **Expect:** Numbered Cue 1.5 becomes current. It is the second Cue in playback order even though its number is 1.5. Group 1 remains on through tracking and Group 3 turns on; Group 2 remains off.
10. Press **GO** again.
    - **Expect:** Numbered Cue 2 becomes current. Groups 1 and 3 track forward, Group 2 turns on, and therefore all twelve dimmers are at 100%.

**Record-operation command-line subcase:** Run this subcase from a separate fresh `cue-001-record-operations.show` copy so deleting a Cue cannot disturb the playback sequence above.

1. Use a separate fresh `default-stage.show` copy, create Group 2 as fixtures 101–108, set its Intensity to 30%, and choose the specified warm color. Press `[REC] [SET] [1] [CUE] [1] [ENTER]` to create Cuelist 1 with Cue 1. Press `[CLR]` twice, set Group 2 Intensity to 70%, and press `[REC] [SET] [1] [CUE] [2] [ENTER]` to create Cue 2.
2. Press `[CLR]` twice. Click Group 2, set Intensity to 80%, and do not touch color. Press `[REC] [+] [SET] [1] [CUE] [2] [ENTER]`. Before Enter, confirm the command line reads `RECORD + SET 1 CUE 2`. Confirm Cue 2 now contains the 80% intensity address while every other previously stored Cue 2 address is retained.
3. Press `[CLR]` twice. Click Group 2 and change only its color. Press `[REC] [-] [SET] [1] [CUE] [2] [ENTER]`. Confirm the command line reads `RECORD - SET 1 CUE 2`, the matching Group 2 color address is removed from Cue 2, and its intensity address remains.
4. Press `[CLR]` twice and verify the programmer has no fixture values and no Group values. Press `[REC] [-] [SET] [1] [CUE] [2] [ENTER]`. Confirm Cue 2 is deleted rather than replaced by an empty Cue. Cue 1 remains because record-minus applies only to the explicit target.
5. Repeat the setup in another fresh copy, then delete Cue 2 with `[DEL] [SET] [1] [CUE] [2] [ENTER]`. Compare the resulting Cuelist body, object events, and resolved engine snapshot with step 4; the two delete forms must have the same result. Also attempt empty-programmer record-minus against the only remaining Cue 1 and confirm it is rejected without changing Cuelist 1.

**Assertions:** Recording to an empty playback creates and assigns exactly one Cuelist, records Cue 1, activates the playback, and jumps to that Cue. Recording to the same playback appends and jumps to Cue 2. After Cue 1.5 is inserted, sequential playback resolves Group states as `1`, then `1 + 3`, then `1 + 2 + 3`; current/next UI, playback API, resolved attributes, and UDP output agree. In the command-line subcase, record-plus replaces only matching incoming addresses, record-minus removes only matching addresses, and empty-programmer record-minus deletes the explicit Cue with the same result as Delete while preserving the only-Cue safeguard.

**Pass condition:** Direct playback recording creates the required Cuelist and assignment, inserted decimal Cues enter numeric playback order without rewriting neighboring deltas, omitted values track forward, and playback reconstruction does not depend on residual programmer state.

## CUE-002 — Cue-only restores the previous state

**Priority:** P0  
**Primary layer:** Rust integration plus selected E2E

**Starting show:** Load canonical `default-stage.show`, immediately Save As `cue-002.show`, and use the active copy for this scenario.

**Detailed procedure:**

1. Create Group 1, set it to 30%, press `[REC]`, and click empty Cuelist 1 to record Cue 1.
2. Clear selection and normal programmer values with two `[CLR]` presses. Set Group 1 to 80%.
3. **UI capability required:** arm **Cue only**, then press `[REC]` and click Cuelist 1 to record Cue 2. There is currently no Cue-only control in the Cuelist or Record Settings UI, so the `@ui` procedure stops here until that control exists.
4. For the API/Rust variant, generate Cue 2's automatic restoration delta from the tracked Cue 1 state, append Cue 2, and append Cue 3 with a change on unrelated fixture 101.
5. Assign Cuelist 1 to page 1 playback 1. From **OFF**, click **GO** three times with assertions after each Cue.
6. Return to **OFF** before each direct-jump case. Invoke Cue 1, Cue 2, and Cue 3 by their explicit Cuelist/Cue address through the API and compare each reconstructed state with the sequential result.
7. **UI capability required:** the Cuelist table currently selects a row for inspection but has no **Go to selected Cue** action. Do not treat clicking a row as a direct jump.

**Assertions:** Cue 2 outputs 80%. Cue 3 restores group 1 to 30%. Direct jumps reconstruct the same state as sequential GO operations.

**Pass condition:** Cue-only restoration is deterministic and independent of navigation history.

## CUE-003 — GO, back, pause, resume, and release

**Priority:** P0  
**Primary layer:** Playwright E2E

**Starting show:** Load canonical `default-stage.show`, immediately Save As `cue-003.show`, and use the active copy for this scenario.

**Detailed procedure and virtual checkpoints:**

1. Create Group 1. Set it to 0%, press `[REC]`, and click empty Cuelist 1 for Cue 1. Clear twice, set Group 1 to 100%, set **Cue Fade** to `4.0 s`, and record to Cuelist 1 for Cue 2.
2. Assign Cuelist 1 to page 1 playback 1. Use that assigned playback's configured **GO** control for Cue 1 and then Cue 2. Do not use the Cuelist View as a playback surface.
3. Call the virtual-clock endpoint with 0 ms and assert byte 0; advance 2,000 ms and assert approximately 128; advance another 2,000 ms and assert 255.
4. Click **OFF**, click **GO** for Cue 1, and click **GO** again for Cue 2. Advance 1,000 ms.
5. **UI capability required:** use explicit **PAUSE** and **RESUME** actions on the assigned playback, advance 10,000 ms while paused, verify no progress, resume, and advance the remaining 3,000 ms. The Cuelist View intentionally contains no playback execution controls. API/Rust coverage uses playback pause/resume actions until assigned playback controls can be configured for these actions.
6. Click **GO −** once and verify Cue 1's tracked state.
7. Click **OFF** and verify restoration to the next authoritative source.

**Assertions:** Current cue, next cue, paused state, transition timestamps, and DMX values match every checkpoint. During the 10,000 ms paused jump, both engine values and packet bytes remain at the paused level.

**Pass condition:** Navigation and pause state use application time, with no progress caused by wall time or a large paused-time jump.

## CUE-004 — Per-value timing, Cue defaults, and forced Cue timing

**Priority:** P1
**Primary layer:** Paired API/UI E2E plus Rust boundary checks

**Starting show:** Load canonical `default-stage.show`, immediately Save As `cue-004.show`, and use the active copy for this scenario.

**Detailed procedure:**

1. Create the standard `default-stage.show` Groups 1–3. Begin with Programmer Fade set to a deliberately different value such as 9 seconds so an explicit command time cannot pass accidentally by matching the global fallback.
2. Select Group 1 and press `[AT] [5] [0] [TIME] [2] [ENTER]`.
   - **Expect:** The command contains `G1 AT 50 TIME 2`. The programmer stores Group 1 Intensity at 50% with an explicit 2-second fade. It ignores the 9-second Programmer Fade both live and when later recorded.
3. Select Group 2, set Intensity to 70% with `[TIME] [4]`, and set Color blue with an explicit 1-second fade and a 1-second value delay. Enter the delay by pressing `[TIME]` twice so the second consecutive press becomes `DELAY`; the complete value command must visibly contain `DELAY 1 TIME 1` or the same clauses in the opposite entry order.
4. Select Group 3, set Intensity to 60%, and press Enter without entering `TIME` or `DELAY`. Confirm this value has no explicit timing override in the programmer.
5. Record the values as Cue 1 with `[REC] [SET] [1] [CUE] [1] [TIME] [3] [ENTER]`. Before Enter, confirm the command reads `RECORD SET 1 CUE 1 TIME 3`.
   Open Cue 1 in the Cuelist View and set its Cue Delay to 0.5 seconds.
   - **Expect:** Cue 1 stores a 3-second master fade and 0.5-second master delay. Group 1 retains its explicit 2-second fade and uses the Cue's 0.5-second delay because it has no value delay. Group 2 Intensity retains its 4-second fade and uses the 0.5-second Cue delay. Group 2 Color retains its explicit 1-second value delay plus 1-second fade. Untimed Group 3 Intensity uses the Cue's 0.5-second delay and 3-second fade. Programmer Fade is not stored in place of any of them.
6. Repeat from a fresh copy while omitting the Cue number: `[REC] [SET] [1] [TIME] [3] [ENTER]` appends the next Cue with the same 3-second master fade. Explicit and append-next forms must produce identical timing data except for the allocated Cue number.
7. Turn the playback off, clear the programmer, run Cue 1, and inspect exact virtual checkpoints:
   - At 499 ms, values without an explicit value delay have not begun because the Cue master delay is active.
   - At 500 ms, Group 1 Intensity, Group 2 Intensity, and Group 3 Intensity begin their fades.
   - At 999 ms, Group 2 Color has not begun because its explicit value delay is still active.
   - At 1,000 ms, Group 2 Color begins its 1-second fade.
   - At 2,000 ms, Group 2 Color reaches its target.
   - At 2,500 ms, Group 1 reaches 50%.
   - At 3,500 ms, untimed Group 3 reaches 60% through Cue master timing.
   - At 4,500 ms, Group 2 Intensity reaches 70%.
8. In Cuelist 1 settings, leave **Force Cue Timing** disabled and repeat the playback to prove the mixed per-value timings above remain authoritative.
9. Enable **Force Cue Timing** and replay Cue 1 from the same source state.
   - **Expect:** Every value waits for Cue 1's 0.5-second master delay and then uses its 3-second master fade. Stored per-value fades and per-value start delays remain present in Cue data but are ignored during this playback because the Cuelist forces Cue timing. All attributes reach their targets together at 3,500 ms.
10. Disable **Force Cue Timing** and replay once more. Confirm the original mixed value timing plus Cue-delay/fade fallback returns without rerecording the Cue.

**Timing-scope rule:** `TIME` and `DELAY` in a value command belong to that individual fixture/group attribute value. `TIME` in a Cue-record command is the Cue's master fade fallback. Cue Delay is edited independently in the Cuelist View and is the start-delay fallback for values without an explicit value delay. A Cue-record `DELAY` clause is reserved for the Cue trigger modes defined in CUE-005; it is not the Cue Delay field or a second spelling for a value's fade-start delay.

**Assertions:** Every programmer value retains its own optional fade and delay. Cue recording preserves those timings and separately stores the Cue master Fade and Delay. Each missing value-time component uses the corresponding Cue fallback. Explicit components ignore that fallback and Programmer Fade. **Force Cue Timing** can temporarily make both Cue master values authoritative without deleting stored per-value timing.

**Pass condition:** Mixed attribute timings survive Cue storage, hit exact delay/fade boundaries, and respond deterministically to the Cuelist's Force Cue Timing setting.

## CUE-005 — GO, FOLLOW, and TIME triggers use Cue completion time

**Priority:** P1
**Primary layer:** Rust integration plus selected E2E

**Starting show:** Load canonical `default-stage.show`, immediately Save As `cue-005.show`, and use the active copy for this scenario.

**Detailed procedure:**

1. Create Group 1 and record Cue 1 with a 2-second fade and the default **GO** trigger. A Cue with no Cue-record `DELAY` clause is stored as **GO** and never starts merely because the previous Cue completed.
2. Record three alternative versions of Cue 2 in independent fresh copies, using the same programmer values:

   | Stored command suffix | Stored trigger | Meaning |
   | --- | --- | --- |
   | no `DELAY` clause | **GO** | Wait indefinitely for the operator to press GO. |
   | `DELAY` with no number, or `DELAY 0` | **FOLLOW** | Trigger immediately when the preceding Cue has finished all of its value delays and fades. |
   | `DELAY 4` | **TIME 4 s** | After the preceding Cue finishes all delays and fades, wait another 4 seconds, then trigger. |

   Enter `DELAY` on the Lightning Desk by pressing `[TIME]` twice consecutively. For example, `[REC] [SET] [1] [CUE] [2] [TIME] [TIME] [4] [ENTER]` must display `RECORD SET 1 CUE 2 DELAY 4` and store Cue 2 with trigger **TIME 4 s**. The zero form displays `DELAY 0` but normalizes to **FOLLOW**.
3. **GO case:** Start Cue 1 at virtual 0. Advance through 2,000 ms and then by seven days. Cue 1 remains current and Cue 2 does not start. Press GO once and confirm Cue 2 starts exactly once at that application timestamp.
4. **FOLLOW case:** Start Cue 1 at virtual 0. At 1,999 ms Cue 1 is still fading and Cue 2 has not started. At exactly 2,000 ms Cue 1 completes and Cue 2 triggers with no extra wait.
5. **TIME case:** Start Cue 1 at virtual 0. Cue 1 completes at 2,000 ms. Cue 2 remains pending through 5,999 ms and triggers exactly at 6,000 ms: 2 seconds for the preceding Cue to finish plus the stored 4-second trigger time.
6. Repeat FOLLOW and TIME with a preceding Cue containing several values whose completion times differ. Define preceding-Cue completion as the latest `value delay + value fade` endpoint after applying **Force Cue Timing** if enabled. Neither automatic trigger may fire when only the first or master-fade value has completed.
7. **UI capability required:** edit each Cue's Trigger through the Cue editor using exactly **GO**, **FOLLOW**, or **TIME**. Choosing **TIME** exposes its numeric wait field. The UI edit and command-line storage paths must serialize identical trigger data.

**Trigger ownership rule:** The trigger setting belongs to the Cue that will start. In the example “Cue 1 fades for 2 seconds, then wait 4 seconds, then the next Cue starts,” Cue 1 is **GO** and Cue 2 is **TIME 4 s**. FOLLOW and TIME measure from the completion of the preceding Cue, not from its initial GO press.

**Assertions:** GO never advances automatically. FOLLOW fires at the exact completion boundary of the preceding Cue. TIME fires at that completion boundary plus its own duration. Large virtual-time jumps produce one transition only, and the Cue editor, command line, persisted Cuelist, playback API, and current/next display agree on the trigger type and duration.

**Pass condition:** Cue triggers distinguish manual GO, immediate FOLLOW, and delayed TIME without depending on wall time or confusing value delay with sequence timing.

## CUE-006 — Select the active playback and use its implicit Cuelist

**Priority:** P1
**Primary layer:** Playwright E2E plus server integration

**Implementation status:** Implemented. Selection is persisted by desk and show, not by user: sessions attached to the same desk share it, while two desks used by the same user remain independent.

**Starting show:** Load canonical `default-stage.show`, immediately Save As `cue-006-active-playback.show`, and use the active copy for this scenario.

**Detailed procedure:**

1. Create Cuelists 1 and 2 with at least one Cue each. Assign Cuelist 1 to page 1 playback 1 and Cuelist 2 to page 1 playback 2.
2. Hold the physical Shift key and press `Z`.
   - **Expect now:** The command line contains exactly `SELECT`; no playback has been selected merely by entering the shortcut.
3. Touch page 1 playback 2.
   - **UI capability required:** Playback 2 becomes the active playback and has one unambiguous active-playback indication. Cuelist 2 is the active Cuelist because it is assigned to that playback.
4. Run playback 1 after selecting playback 2, then press `[SHIFT] [4]`.
   - **Expect:** The Cue details for playback 2/Cuelist 2 open. Running another playback does not silently replace the explicit active-playback selection.
5. Close the details, put a distinct value into the programmer, and press `[REC] [CUE] [7] [ENTER]` without entering a playback address or a Cuelist Pool number.
   - **UI/server capability required:** Cue 7 is recorded in Cuelist 2, the Cuelist assigned to active playback 2. Cuelist 1 is unchanged.
6. Enter a complete explicit address targeting Cuelist 1 and record another distinct Cue.
   - **Expect:** The explicit Cuelist address overrides the active-playback default without changing which playback is active.

**Assertions:** Shift-Z produces `SELECT` without executing selection. Touching a playback establishes exactly one active playback for the operator session. Shift-4 and an address-omitting `[REC] [CUE] <number> [ENTER]` both resolve through that same selection. Playback execution order never changes it implicitly, and an explicit playback or Cuelist address takes precedence.

**Pass condition:** The active playback is a deliberate operator choice and is the single shared default for Cue details and Cue recording when no playback/Cuelist address is supplied.

## CUE-014 — Go To and Load a Cue on a concrete playback

**Priority:** P0
**Primary layer:** Visible Playwright E2E plus Rust playback/server integration

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `cue-014-go-to-load.show`, create one three-Cue Cuelist with visible tracked intensity changes, assign it to two concrete playbacks, and assign both to page 1.

**Detailed procedure:**

1. Enter `SELECT` with physical Shift-Z and touch playback 2. Verify the touch selects it without firing its button, and then run playback 1. The explicit selection remains playback 2.
2. Return to the programmer keypad and enter `[CUE] 3 [ENTER]`. Playback 2 activates, its fader becomes full, and Cue 3 becomes current using normal effective timing. Playback 1 remains an independent instance even though both target the same Cuelist.
3. Enter `[CUE] [CUE] 2 [ENTER]`. Current Cue, fader, activation, and DMX do not change. Every playback-state surface reports Cue 2 as a loaded effective next Cue distinct from the ordinary next Cue.
4. Press forward GO. Cue 2 executes, the Load is consumed, and ordinary sequence progression resumes from Cue 2. Load another Cue, press GO minus, and verify the Load remains. Press Off and verify it clears.
5. Repeat Go To and Load with explicit pool and page/playback forms. Try a missing Cue, missing selection on another desk, missing playback, and a bare Cuelist shared by multiple playbacks. Every rejection is atomic.
6. Repeat Go To under Grand Master 50% and Blackout. Confirm neither control is bypassed. Reopen the show after Load and verify transient loaded state is not persisted.

**Assertions:** Stable Cue identity survives renumbering, deleting a loaded Cue clears it, API and playback UI expose current/normal-next/effective-next/loaded state, and same-user sessions share selection only when attached to the same desk.

**Pass condition:** Go To and Load operate on exactly one concrete playback with tracked timing and output safety, and all UI/API/OSC feedback agrees with authoritative runtime state.

## CUE-007 — Explicit tracked-off values block an inserted on Cue

**Priority:** P0
**Primary layer:** Paired API/UI E2E

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `cue-007-tracked-off.show`, and use the active copy for this scenario.

**Detailed procedure:**

1. Create the twelve-dimmer Groups 1–3. Record Cue 1 with Group 1 Intensity at 100%.
2. Clear twice, explicitly set Group 1 Intensity to 0%, and record Cue 2. This Cue must contain a stored zero value, not merely omit Group 1 because it already happens to be dark.
3. Clear twice, set Group 2 Intensity to 100%, and record Cue 3. Play Cues 1–3 from **OFF** and confirm Group 1 is on in Cue 1, explicitly turns off in Cue 2, and remains off by tracking in Cue 3 while Group 2 turns on.
4. Clear twice, explicitly set Group 1 to 0% again, and record Cue 4 even though Group 1 is already off in the tracked state. Confirm Cue 4 contains the repeated explicit zero.
5. Clear twice, set Group 3 to 100%, and record Cue 5. Confirm Group 1 remains off in Cue 5 through Cue 4's explicit zero.
6. Turn the playback off. Clear twice, set Group 1 to 100%, and insert Cue 3.5 with the explicit Cue-address command.
7. Replay from **OFF**. At Cue 3, Group 1 is off and Group 2 is on. GO to Cue 3.5 and confirm Group 1 turns on. GO to Cue 4 and confirm its repeated explicit zero turns Group 1 off again. GO to Cue 5 and confirm Group 1 remains off while Groups 2 and 3 are on.
8. Inspect the stored deltas: Cue 2 contains Group 1 at 0%, Cue 3 does not contain Group 1, Cue 3.5 contains Group 1 at 100%, and Cue 4 contains Group 1 at 0%. No optimization may discard Cue 4's apparently redundant zero.

**Assertions:** A deliberately recorded zero is an attribute value and a tracking block. Inserting an on Cue between two explicit off Cues changes the state only from the inserted Cue until the later explicit off Cue; all following Cues track from that later off value.

**Pass condition:** Repeated explicit off values survive storage and reliably stop newly inserted earlier changes from tracking beyond them.

## CUE-008 — Recording in Preload does not activate the playback

**Priority:** P0
**Primary layer:** Paired API/UI E2E

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `cue-008-preload-record.show`, and use the active copy for this scenario.

**Detailed procedure:**

1. Create the twelve-dimmer Groups 1–3 and confirm page 1 playback 1 is empty and no playback is active.
2. Enter Preload mode, select Group 1, set Intensity to 100%, press `[REC]`, and touch a button belonging to page 1 playback 1.
3. Confirm that recording still creates a new Cuelist, stores Cue 1, and assigns it to playback 1, exactly as normal direct playback recording does.
4. Confirm the crucial Preload difference: playback 1 remains off, no Cue is current, the stored Cue does not contribute to live output, and fixtures 1–4 remain at their prior live values. The UI may show Cue 1 as ready/next, but not active.
5. Leave Preload mode without executing Preload GO. Turn playback 1 on normally and confirm it starts at Cue 1 and Group 1 reaches 100%.
6. From an independent normal-programmer copy, repeat the same Record target without Preload mode and confirm the playback activates and jumps to Cue 1. Compare the two stored Cuelists and prove that only activation/runtime state differs.

**Assertions:** Preload recording persists the same Cue and playback assignment as normal recording while leaving playback runtime and audience-facing output unchanged.

**Pass condition:** An operator can prepare and store a Cue in Preload without accidentally activating its playback.

## CUE-009 — Plain and status Move/Copy preserve their distinct meanings

**Priority:** P1
**Primary layer:** Rust/API integration plus selected UI E2E

**Implementation status:** Specification only until Cue Move/Copy execution and the post-Enter choice modal exist on the production command surface.

**Starting show:** Load canonical `compact-rig.show`, immediately Save As a separate `cue-009-<operation>.show` copy for each operation below, and use only that active copy.

**Setup:** Create the twelve-dimmer Groups 1–3. In Cuelist 1, record Cue 1 with Group 1 at 100%, Cue 2 with only Group 2 at 100%, and Cue 3 with Group 1 at 0%. The reconstructed status at Cue 2 is therefore Groups 1 and 2 at 100%, although Cue 2's own command/delta contains only Group 2. Create Cuelist 2 with Cue 1 setting Group 1 to 0% and Group 3 to 100%, so its tracked history differs from Cuelist 1.

**Detailed procedure:**

1. Enter a complete command such as `[CPY] [SET] [1] [CUE] [2] [AT] [SET] [2] [CUE] [2] [ENTER]` or the equivalent `[MOV]` command.
2. After Enter, require a modal with exactly the applicable choices **Plain Copy** and **Status Copy**, or **Plain Move** and **Status Move**, plus **Cancel**. Enter alone must not guess which semantic the operator intended. Cancel closes the modal and changes neither Cuelist.
3. Run each of these four operations from an independent setup copy:

   | Operation | Source after operation | Destination Cue contents |
   | --- | --- | --- |
   | Plain Copy | Cue 2 remains in Cuelist 1 | Copy only Cue 2's stored Group 2 intensity command/delta. |
   | Plain Move | Cue 2 is removed from Cuelist 1 | Move only Cue 2's stored Group 2 intensity command/delta. |
   | Status Copy | Cue 2 remains in Cuelist 1 | Materialize the complete tracked status at source Cue 2 for every attribute touched up to that point: Group 1 at 100% and Group 2 at 100%. |
   | Status Move | Cue 2 is removed from Cuelist 1 | Materialize the same complete tracked source status at the destination. |

4. Replay destination Cuelist 2. A Plain destination Cue inherits Group 1 at 0% from Cuelist 2 and adds Group 2 at 100%; a Status destination Cue explicitly restores the source-point status, so Groups 1 and 2 are at 100%. Group 3 is untouched by the source status and continues to track from destination Cue 1.
5. For both Move variants, replay Cuelist 1 after source Cue 2 is removed and verify its tracking is recalculated from the remaining Cue deltas. For both Copy variants, prove Cuelist 1 is byte-for-byte unchanged.

**Assertions:** Plain operations transfer only the selected Cue's stored commands. Status operations materialize the reconstructed state of every attribute touched at or before the source Cue, but do not invent values for untouched attributes. Copy retains the source Cue; Move removes exactly that Cue. All four operations insert the destination at its numeric Cue position and recalculate tracking from stored data.

**Pass condition:** Move versus Copy controls source retention, while Plain versus Status independently controls whether the destination receives the Cue delta or its complete tracked source-point status.

## CUE-010 — Tracking is per attribute and respects newer programmer LTP

**Priority:** P0
**Primary layer:** Paired API/UI E2E plus engine arbitration checks

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `cue-010-attribute-tracking.show`, and use the active copy for this scenario.

**Setup:** Overwrite Group 1 with RGB fixtures 21–24 in order. Use exact named colors whose encoded RGB values are asserted by the shared output oracle.

**Detailed procedure:**

1. Record Cue 1 with only Group 1 Intensity at 100%. Do not touch or record Color.
2. Clear twice and record Cue 2 with only Group 1 Intensity at 50%. Confirm neither Cue contains a Group 1 Color address.
3. Turn the playback off. In the normal programmer set Group 1 Color to green without setting Intensity, and leave that programmer value active.
4. Run Cue 1 and then Cue 2. The Cues control Intensity, but the programmer's green remains unchanged because neither Cue has introduced a Color contribution. Navigating an intensity-only Cuelist must not clear, default, or claim Color.
5. Turn the playback off and clear the programmer. Set only Group 1 Color to blue and record Cue 3 without touching Intensity. Clear twice, re-enter the green Group 1 programmer Color, and leave it active. Restart the playback from Cue 1 so green is newer than Cue 3's recording operation but older than Cue 3's later execution.
6. Run Cue 1 and Cue 2 again, then GO to Cue 3. At Cue 3, the newly executed blue Cue contribution is the newer LTP edit for Color and wins. Intensity remains the tracked 50% from Cue 2.
7. Append Cue 4 with an unrelated Group 2 intensity change. GO to Cue 4 and confirm blue tracks forward for Group 1 Color while Group 1 Intensity remains at 50%.
8. Turn the playback off. Confirm the still-active green programmer Color is revealed again without an intervening default-color frame.

**Assertions:** Cue storage and tracking operate on fixture/group attribute addresses, not entire fixtures. An omitted Color address is not tracked before any Cue introduces it and cannot overwrite a programmer Color. Once a Cue introduces Color, that LTP contribution tracks until another Color value changes or the playback is released. Intensity and Color ownership are resolved independently.

**Pass condition:** Playback changes only attributes actually stored in its Cues, and per-attribute LTP arbitration reveals the correct underlying source when the playback is released.

## CUE-011 — Cuelist View selects and edits Cues without executing them

**Priority:** P0
**Primary layer:** Paired API/UI E2E plus layout assertions

**Implementation dependency:** Implement the [Cuelist View and Cuelist Settings contract](../planned%20features/06-cuelist-view-and-settings.md) before enabling this UI scenario.

**Starting show:** Load canonical `compact-rig.show`, immediately Save As `cue-011-cuelist-view.show`, create Cuelist 1 with Cues 1, 2, and 3, assign it to playback 1, and use the active copy.

**Detailed procedure:**

1. Open the **Cuelist Pool**, tap Cuelist 1, and confirm the destination is titled as the **Cuelist View** for Cuelist 1 rather than another pool or playback surface.
2. Inspect the Cue table header. It contains exactly **Preview**, **No.**, **Name**, **Trigger**, and **Fade** in that order. It has no **Status** column and no `Active` or `Tracked` cells masquerading as Cue data. Current/next row styling may still be present without adding a column.
3. Confirm the Cuelist View contains no GO, GO minus, Toggle, or Off buttons. Those actions belong to assigned playback controls.
4. Record the active playback state, current Cue, output frame, object revision, and playback-event mark. Tap the row for Cue 2 once.
   - **Expect:** Cue 2 becomes the selected row and its fields appear at the top of the right-side Cue editor. The active Cue, playback revision, output, and event stream do not change. Selecting is not executing or jumping.
5. Repeat row selection with keyboard focus plus Enter and Space. Both gestures remain selection-only.
6. Edit the selected Cue's Title, Fade, Delay, and Trigger fields. Use `Center transition`, Fade `2.5` seconds, Delay `1.25` seconds, and each Trigger choice in independent copies: GO, FOLLOW, and TIME `4` seconds.
7. Commit each field with Enter and repeat through blur. Confirm the revision-checked update stores exact milliseconds without rounding drift, updates the table row immediately, and still produces no playback action.
8. Select another Cue and return to Cue 2. Confirm every field reloads from the selected Cue rather than leaking editor state from the previously selected row.
9. Use the **Cuelist Settings** action in the Cuelist View title bar. Confirm it opens settings for Cuelist 1. Close it without changes and verify Cue selection remains Cue 2.
10. Save, reload the show, reopen Cuelist 1, and confirm the edited Cue title, fade, delay, trigger, and trigger time persist.
11. Submit an invalid negative Fade/Delay and a stale revision in independent cases. Show a visible error, preserve the last valid Cue bytes, and do not execute any Cue.

**Renumber Cues subcase:**

1. From a fresh copy, create Cues numbered `1`, `1.5`, `2`, and `7` with distinct titles and stored values. Keep Cue 1.5 selected in the editor and active on playback 1.
2. Open **Cuelist Settings**, click **Renumber Cues**, leave **Start Cue** empty, and press Enter.
   - **Expect:** One atomic revision changes the numbers to `1`, `2`, `3`, and `4` in the same row order. The former Cue 1.5 is now Cue 2, remains selected and current, retains its exact data, and produces no playback execution or output change.
3. Repeat from an independent copy, enter Start Cue `10`, and confirm with the modal button.
   - **Expect:** The numbers become `10`, `11`, `12`, and `13`. Current/next labels and explicit Cue addressing use the new numbers immediately; the old decimal/number addresses no longer identify those Cues.
4. Reopen the modal and click Cancel, then repeat by closing the backdrop/Escape. Confirm both paths preserve the exact Cuelist bytes and revision.
5. Submit `0`, a negative number, a fractional number, an overflowing number, and a stale expected revision in independent copies. Each request shows a visible error and leaves every Cue number unchanged; no partial prefix may be renumbered.
6. Renumber a one-Cue Cuelist to Start Cue `10` and confirm its only Cue becomes Cue 10. Confirm the button is unavailable only when there is no open Cuelist or no Cue to renumber.
7. Save and reload both successful arrangements. Confirm numbers, selected-row restoration policy, active playback recovery policy, Cue contents, tracking, and previews remain valid.

**Assertions:** Table columns and right-side controls match the named contract. Row selection, editing, and renumbering never execute a Cue. Cue edits are revision checked and survive reload. Renumber Cues performs one atomic order-preserving mutation from Cue 1 or a custom positive whole-number start, preserves the selected/current Cue and all Cue data, and cannot leave partial results. Playback action buttons and the Status column are absent.

**Pass condition:** The Cuelist View is a safe Cue inspection/editor surface, and Cuelist Settings can renumber the sequence transactionally without changing its contents, order, active state, or output.

## CUE-012 — Cuelist Settings control Chaser, priority, wrapping, restart, and timing

**Priority:** P1
**Primary layer:** Rust/API integration plus paired UI E2E

**Implementation dependency:** Implement the [Cuelist View and Cuelist Settings contract](../planned%20features/06-cuelist-view-and-settings.md) before enabling this scenario. Reuse the exact virtual-time phase oracle from TIME-003 for Chaser timing.

**Starting show:** Use a separate fresh `cue-012-<case>.show` copy for every case below.

**Settings surface:** Open a Cuelist View and click **Cuelist Settings** in its title bar. The same persisted settings must appear if opened through any retained Cuelist Pool shortcut.

### Chaser case

1. Create four Cues with distinguishable output states and set Mode to **Chaser**.
2. Set Chaser X-fade to 100 ms, Speed Group A to 120 BPM, and multiplier to `1×`. Start at a marked virtual timestamp.
   - **Expect:** The effective step duration is 500 ms. Each 100 ms X-fade and step boundary is exact and produces the same result for one large virtual jump as for equivalent smaller advances.
3. Change the multiplier to `0.5×` and confirm the rate is half as fast: the step duration becomes 1,000 ms. Change it to `2×` and confirm the rate doubles: the step duration becomes 250 ms.
4. Change Speed Group A BPM while midway through a step and assert the recalculated next boundary and continuous phase using TIME-003's oracle.
5. Attempt an X-fade longer than the effective step interval. The UI and API reject it visibly and preserve the prior valid value rather than silently creating an undefined overlap.

### Intensity priority-mode case

1. Create two equal-numeric-priority Cuelists that overlap on Group 1 Intensity. Let Cuelist A contribute 80%, then trigger Cuelist B later with 30%.
2. Set both to Intensity mode **HTP**. Confirm 80% wins even though B is newer.
3. Set both to Intensity mode **LTP** and retrigger B. Confirm the newer 30% wins even though it is lower.
4. Add conflicting Color values. Confirm Color remains LTP in both Intensity modes.
5. Give A a higher numeric priority and repeat. Confirm numeric priority resolves first, so lower-priority B cannot win through HTP magnitude or LTP recency.

### Wrap Around case

1. Create Cue 1 with Group 1 at 100% and Cue 2 with only Group 2 at 100%. Give Cue 1 a 1-second Cue Delay and 2-second Cue Fade. Keep an unrelated lower source contributing Group 2 at 20% so reset restoration is observable.
2. With Wrap Around **Off**, run through Cue 2 and press GO again. Cue 2 remains current and the tracked state remains Group 1 at 100%, Group 2 at 100%.
3. From an independent copy with Wrap Around **Tracking**, press GO at Cue 2. Cue 1 becomes current as the next numeric step, but Group 2 remains at 100% because its final-Cue value tracks across the boundary and Cue 1 does not change it.
4. From an independent copy with Wrap Around **Reset**, press GO at Cue 2.
   - Cue 1 becomes current.
   - During Cue 1's 1-second Delay, the prior state remains visible.
   - Over Cue 1's 2-second Fade, Group 1 transitions to its Cue 1 value and the Cuelist's Group 2 contribution releases because Cue 1 contains no Group 2 value.
   - Group 2 reveals the unrelated 20% source at the fade endpoint without a hard-zero frame.
5. Confirm no Wrap Around mode changes GO minus behavior at Cue 1; reverse wrapping remains undefined until a separate contract exists.

### Restart-mode case

1. Create Cues 1, 2, and 3 with distinguishable tracked states. Run through Cue 2, record its reconstructed output, and turn the Cuelist Off.
2. Leave Restart mode at its default **First Cue** and turn the Cuelist on with ON. Confirm Cue 1 becomes current and output is reconstructed from Cue 1 rather than Cue 2's former tracked state.
3. Repeat from independent copies using Toggle-to-On and GO with GO-activates enabled. Every activation path uses Cue 1 in First Cue mode.
4. Set Restart mode to **Continue Current Cue**, run through Cue 2, and turn the Cuelist Off. Turn it on again.
   - **Expect:** Cue 2 becomes current again with the exact tracked state it had immediately before Off. Turning on does not advance to Cue 3.
5. Repeat Continue Current Cue through ON, Toggle-to-On, and GO-activates. All three restore Cue 2.
6. From a never-run Cuelist in Continue Current Cue mode, turn it on and confirm it starts at Cue 1. From another copy, remember Cue 2, turn Off, delete Cue 2, and turn on again; confirm the deterministic fallback is Cue 1 rather than Cue 3 or an invalid index.

### Disable-timing case

1. Create Cue 1 with a 1-second Cue Delay, 2-second Cue Fade, and at least one value with different explicit timing. Create Cue 2 with trigger TIME 4 seconds. Record the complete serialized Cue/Cuelist timing body before changing settings.
2. With **Disable Cue Timing** off and Force Cue Timing off, execute the Cues and verify every normal value, Cue, and trigger boundary from CUE-004/CUE-005.
3. Enable **Disable Cue Timing** and execute from the same source state.
   - Every per-value and Cue Delay/Fade is treated as zero.
   - Cue 2's TIME wait is treated as zero, so it follows immediately after Cue 1's now-instant completion.
   - The transition produces no intermediate fade frames while still emitting the normal revisions and execution events.
4. Enable Force Cue Timing while Disable Cue Timing remains enabled. Confirm Disable Cue Timing has precedence and execution remains immediate.
5. Disable the timing bypass and execute again. Confirm every originally stored duration returns without rerecording or editing a Cue, and compare the serialized timing body byte-for-byte with step 1.
6. Repeat with a Chaser. Disable Cue Timing makes Chaser X-fade a snap but retains Speed Group/multiplier step cadence; it must not collapse the Chaser into an unbounded zero-duration loop.

### Persistence and migration

Save and reload every configuration. Confirm Mode, X-fade, Speed Group, multiplier, Intensity mode, numeric priority, Force Cue Timing, Disable Cue Timing, Wrap Around, and Restart mode persist. Load legacy copies with `looped: false` and `looped: true`; confirm they migrate to Off and Tracking respectively, default Restart mode to First Cue, default Disable Cue Timing to off, and preserve Cue data and assignments.

**Assertions:** Chaser rate derives from BPM and multiplier, Chaser X-fade is validated, numeric priority remains distinct from HTP/LTP Intensity mode, non-intensity attributes remain LTP, Off/Tracking/Reset wrapping produces the defined tracked or released state, Restart mode chooses First Cue or the remembered current Cue, and Disable Cue Timing bypasses durations without mutating them.

**Pass condition:** Cuelist Settings are persisted engine behavior rather than UI-only labels, and every mode produces deterministic playback, arbitration, wrapping, restart, and timing-bypass results.

## MIB-001 — A dark fixture prepositions for its next lit Cue

**Priority:** P0
**Primary layer:** Rust/API timing checks plus paired UI E2E

**Implementation dependency:** Implement the [Move in Black contract](../planned%20features/07-move-in-black.md) before enabling this scenario. The test must inspect normalized MIB runtime state as well as output DMX.

**Starting show:** Load canonical `default-stage.show`, immediately Save As `mib-001-basic.show`, and use moving fixtures 101 and 102 as enabled and disabled comparison fixtures.

**Patch setup:**

1. Open **Setup → Patch** and confirm every migrated/new fixture defaults to **Move in Black: On** and **MIB Delay: 0 s**.
2. Set fixture 101 to Move in Black On with a 1-second delay. Set fixture 102 to Move in Black Off with the same stored delay. Save, reload, and confirm both fixture-level settings persist.

**Cue setup:**

1. Record Cue 1 with fixtures 101 and 102 at 100% Intensity and exact Position A.
2. Record Cue 2 with both fixtures fading to 0% over 2 seconds. Do not record a Position change in Cue 2.
3. Record Cue 3 with both fixtures at 100% and exact Position B. Give the Position change an explicit 3-second fade so hidden movement boundaries are deterministic.
4. Clear the programmer, turn the Cuelist off, and start Cue 1. Confirm both fixtures are lit at Position A.

**Exact Move in Black checkpoints:**

1. Trigger Cue 2 at virtual 0.
2. At 1,999 ms, both resolved Intensities remain above zero and both fixtures remain at Position A. Fixture 101 has no dark-since timestamp and its MIB delay has not started.
3. At exactly 2,000 ms, both resolved Intensities reach zero. Fixture 101 records dark-since 2,000 ms and an MIB delay deadline of 3,000 ms. Fixture 102 remains MIB-disabled.
4. At 2,999 ms, fixture 101 remains exactly at Position A.
5. At 3,000 ms, fixture 101 begins its hidden 3-second Position transition toward B. Cue 2 remains current and Cue 3 has not executed.
6. At 4,500 ms, fixture 101 is exactly halfway between A and B according to the production Position encoder. Fixture 102 remains at A.
7. At 6,000 ms, fixture 101 reaches Position B while still at zero Intensity. Fixture 102 remains at Position A.
8. Trigger Cue 3. Fixture 101 raises Intensity from Position B without jumping backward or restarting its completed Position move. Fixture 102 begins the normal visible 3-second transition from A to B because its Move in Black setting is disabled.

**Resolved-dark blocking subcase:**

1. Repeat from a fresh copy, but keep fixture 101 at 20% from a separate programmer or higher/equal-authority playback while Cue 2 reaches its stored 0%.
2. Advance well beyond the nominal Cue 2 fade and MIB delay. Confirm MIB remains blocked because actual resolved Intensity is above zero.
3. Release the last light-producing contribution at a marked timestamp. Confirm resolved Intensity reaches zero then, dark-since begins then, and the complete 1-second MIB delay is measured from that later timestamp.
4. Raise Intensity above zero during the delay in another copy. Confirm the pending MIB is cancelled. Return to zero and prove a fresh full delay starts rather than resuming the old one.

**Future-state and invalidation subcase:**

1. Insert another dark Cue between Cues 2 and 3. Confirm fixture 101 still targets the next eventual lit state at Position B while the first dark Cue is current.
2. While dark and before movement completes, edit the future lit Cue to Position C. Confirm the pending target is recalculated without writing derived Position values into Cue 2.
3. Delete the only future lit Cue or release the Cuelist. Confirm the pending MIB contribution disappears and the correct underlying Position source is revealed without an intermediate default frame.

**Assertions:** MIB is enabled by default per fixture, its delay starts only when resolved Intensity actually reaches zero, disabled fixtures do not preposition, hidden Position timing comes from the next lit Cue, competing light sources block it, interruption restarts the safety delay, and Cue edits invalidate runtime targets without mutating stored Cue data.

**Pass condition:** A moving fixture uses its dark time to reach the next lit Position predictably, while patch settings, arbitration, exact timing, and cancellation prevent premature or visible movement.

## CUE-013 — Deleting inactive and active Cues preserves deterministic playback

**Priority:** P0
**Primary layer:** Paired API/UI E2E plus playback runtime integration

**Implementation dependency:** Implement the [active-Cue deletion contract](../planned%20features/06-cuelist-view-and-settings.md#deleting-a-cue-including-the-active-cue) before enabling this scenario.

**Starting show:** Load canonical `compact-rig.show`, immediately Save As a separate `cue-013-<case>.show` copy for each case, and create Groups 1–3 as fixtures 1–4, 5–8, and 9–12.

**Cue setup:**

1. Record Cue 1 with Group 1 Intensity at 100%.
2. Record Cue 2 with only Group 2 Intensity at 100%, so its reconstructed output is Groups 1 and 2 on.
3. Record Cue 3 with only Group 3 Intensity at 100%, so before deletion its reconstructed output would contain all three Groups.
4. Assign the Cuelist to playback 1 and clear the programmer.

**Inactive-Cue deletion:**

1. Start Cue 1 and capture playback state/output. Delete inactive Cue 3 with `[DEL] [SET] [1] [CUE] [3] [ENTER]`.
2. Confirm Cue 3 disappears in one revision, Cue 1 remains current, and playback output, transition timestamps, source ownership, and DMX do not change.
3. Repeat through page-playback addressing and empty-programmer Record-minus. All deletion surfaces produce the same Cuelist and runtime result.

**Active-Cue deletion and GO:**

1. From a fresh copy, run Cue 1 and then Cue 2. Confirm Cue 2 is current and Groups 1 and 2 are at 100%.
2. Mark output/events, then delete active Cue 2 with `[DEL] [SET] [1] [CUE] [2] [ENTER]`.
   - Cue 2 disappears from persisted Cuelist data and the Cuelist View immediately.
   - The playback continues outputting Cue 2's reconstructed held state: Groups 1 and 2 remain at 100%, Group 3 remains off.
   - No fade, release, GO, restart, transient zero frame, or output slot/value change is caused by deletion itself; periodic packet sequence numbers may still advance normally.
   - Runtime diagnostics report a deleted-active Cue 2 anchor with previous Cue 1 and next Cue 3.
3. Press GO once. Confirm Cue 3 becomes current. Reconstruct it from the modified list `[1, 3]`: Group 1 tracks from Cue 1, Group 3 turns on from Cue 3, and Group 2 releases because its only stored change was in deleted Cue 2. Apply Cue 3's normal effective timing.
4. Press GO minus. Confirm Cue 1 becomes current because it is now the previous surviving Cue before Cue 3; the deleted Cue 2 is not recreated or visited.

**Active-Cue deletion and GO minus:**

1. From another fresh copy, run Cue 2, delete it while active, and confirm the same held output/anchor.
2. Press GO minus without pressing GO first. Confirm Cue 1 becomes current and output reconstructs Cue 1 normally.

**Index and safeguard cases:**

1. Run Cue 3, delete inactive Cue 1, and confirm Cue 3 stays current despite its array index changing.
2. Attempt to delete the sole Cue in a one-Cue Cuelist through Delete and empty-programmer Record-minus. Both reject with the same safeguard and preserve runtime/output.
3. Save and reload after deleting an inactive Cue and confirm persistence. Do not persist the deleted-active held snapshot as a replacement Cue; application-restart handling of a live deleted-active anchor follows the explicit playback-recovery policy rather than inventing Cue data.

**Assertions:** Inactive deletion does not disturb current playback. Active deletion removes only the Cue object while holding its reconstructed output until navigation. GO chooses the next surviving Cue, GO minus chooses the previous surviving Cue, later tracking reconstructs from the modified Cuelist, index shifts do not move the current Cue, and the sole-Cue safeguard remains atomic.

**Pass condition:** An operator can delete any non-sole Cue from a running Cuelist without an output discontinuity, and subsequent GO/GO-minus navigation behaves as if the deleted Cue occupied only a temporary navigation anchor.

## MERGE-001 — Two programmers compete by priority and recency

**Priority:** P0  
**Primary layer:** Rust/API integration

**Starting show:** Load canonical `default-stage.show`, immediately Save As `merge-001.show`, and use the active copy for this scenario.

**Detailed harness cases:**

1. Create two authenticated users/sessions, retain both programmer IDs, and set both priorities to the case's stated value through the programmer fixture/API seam.
2. Have programmer A set the same fixture's intensity to 40%, then programmer B set it to 70%; emit a 0 ms frame and prove equal-priority HTP resolves to 70%.
3. Repeat with the documented unequal priorities and prove the higher-priority scope wins.
4. Set the same LTP attribute from A, advance virtual time by 1 ms, then edit it from B. Emit two 0 ms frames without further edits and prove B remains owner on both renders.
5. Release B's winning contribution through the API and emit another frame; prove A's contribution is revealed without a transient default frame.
6. **Harness only:** the UI has no control for programmer priority or for choosing between two simultaneous browser programmers, so this is not a manual single-desk gesture sequence.

**Assertions:** For every case, compare the complete ordered contribution set, chosen source per attribute, normalized resolved value, and application edit timestamp. Re-rendering without mutation must produce an identical result.

**Pass condition:** Priority, HTP, and LTP decisions use operator edit time and never render-loop timing.

## MERGE-002 — Programmer and two playbacks arbitrate correctly

**Priority:** P1  
**Primary layer:** Rust integration plus selected E2E

**Starting show:** Load canonical `default-stage.show`, immediately Save As `merge-002.show`, and use the active copy for this scenario.

**Detailed procedure:**

1. Create Group 2. Record one Cuelist whose Cue contributes intensity and warm color, and a second Cuelist whose Cue contributes a different intensity and position to overlapping Group 2 fixtures.
2. Assign the two Cuelists to page 1 playbacks 1 and 2 using `[SET]`, the Cuelist Pool cell, and the corresponding **Assign Cuelist …** fader target.
3. Click **GO** on playback 1, then playback 2. Click Group 2 and program the third intensity/color/position contribution. Emit a frame and capture every source contribution and winner.
4. Move playback 1's master to 50%, emit a frame, then move it back to 100% and emit another.
5. Click **OFF** for playback 2, emit a frame; clear the programmer in its documented stages, emit a frame; then click **OFF** for playback 1 and emit a frame.
6. Reset all sources and repeat activation in the order programmer, playback 2, playback 1.
7. **UI capability required:** repeat with direct Cue jumps only after the Cuelist surface provides an explicit **Go to selected Cue** action. API/Rust coverage may address the Cues directly now.

**Independent sequences, programmer override, and retrigger subcase:**

1. From a fresh `merge-002-independent.show` copy, create Group 2 from RGB fixtures 101–108 and Group 3 from RGB fixtures 201–205. Create Sequence A on playback 1 with Group 2 at 60% in blue. Create Sequence B on playback 2 with Group 3 at 40% in warm white. Neither Sequence may contain an address belonging to the other Group.
2. Turn on playback 1, then playback 2. Confirm both are active simultaneously: Sequence A controls only Group 2 and Sequence B controls only Group 3. Starting Sequence B must not release, restart, dim, recolor, or otherwise mutate Sequence A.
3. At the same priority, use the programmer to set only Group 2 Color to red. Confirm the newer programmer LTP contribution changes Group 2 from blue to red while Group 3 remains warm white from Sequence B.
4. Trigger Sequence A's current Cue again so its blue Group 2 Color receives a newer application edit timestamp. Confirm Group 2 returns to blue and Group 3 remains unchanged. The retrigger must not claim attributes absent from Sequence A.
5. Edit Group 3 Color in the programmer and then retrigger Sequence A. Confirm Sequence A still cannot overwrite Group 3 because it has no Group 3 Color address, even though Sequence A is now the newest playback action.
6. Release the programmer and each playback one at a time. At every release, assert that only addresses contributed by that source fall back and that the other independent Sequence remains active.

**Priority matrix:** Repeat the overlapping Group 2 Color case from independent copies.

| Programmer priority | Sequence A priority | Expected winner after programmer edit | Expected winner after Sequence A retrigger |
| --- | --- | --- | --- |
| Equal | Equal | Programmer, because its LTP edit is newer | Sequence A, because its retrigger is now newer |
| Higher | Lower | Programmer | Programmer; lower-priority recency cannot win |
| Lower | Higher | Sequence A | Sequence A |

Repeat the matrix for Intensity. Within the winning priority, Intensity uses HTP and LTP attributes use edit recency. A lower-priority source cannot win merely by having the higher intensity or newer timestamp. Record the winning source, priority, application edit timestamp, normalized value, and exact DMX for every cell.

**Assertions:** Independent Cuelists coexist and affect only their stored attribute addresses. At equal priority, the newest programmer or playback execution wins overlapping LTP attributes; retriggering a Sequence refreshes only the addresses actually present in that Cue. Unequal priority is resolved before HTP magnitude or LTP recency. Intensity follows HTP within the winning priority. Releasing a source restores the correct underlying value without a transient zero frame.

**Pass condition:** Multiple Sequences mix without cross-talk, priority is authoritative, equal-priority programmer and playback LTP ownership follows real edit/retrigger order, and every release restores the next valid source.

## MERGE-003 — Full LTP overwrite releases a playback, but Flash and Temp restore it

**Priority:** P0
**Primary layer:** Rust integration plus paired API/UI E2E

**Starting show:** Load canonical `compact-rig.show`, immediately Save As a fresh `merge-003-<case>.show` copy for each case, and use only that active copy.

**Setup:** Overwrite Group 1 with RGB fixtures 21–24. Create Cuelist A with a Cue setting Group 1 Color to blue and assign it to playback 1. Create Cuelist B with a Cue setting the same Group Color addresses to red and assign it to playback 2. Set both masters to 100% and use equal playback priority unless a case says otherwise.

**Detailed cases:**

1. Leave **Switch Cuelist off when fully overwritten** enabled for playback 1. Turn playback 1 on and confirm blue. Then turn playback 2 on normally and confirm red.
   - **Expect:** Playback 2's newer red LTP contribution fully covers every attribute contributed by playback 1, so playback 1 automatically switches off. Its runtime state, visible button state, playback API, and any external feedback all report Off.
2. Repeat with playback 1 contributing both blue Color and Intensity while playback 2 contributes only red Color.
   - **Expect:** Playback 1 remains on because only its Color is overwritten; its Intensity contribution is still authoritative. A partial attribute overwrite must never count as fully overwritten.
3. Disable **Switch Cuelist off when fully overwritten** for playback 1 and repeat the full Color overwrite.
   - **Expect:** Playback 1 remains enabled even while playback 2 owns the visible Color. Turning playback 2 off reveals playback 1's blue immediately.
4. Re-enable automatic switch-off. Turn playback 1 on, then press and hold playback 2's configured **FLASH** button.
   - **Expect while held:** Red temporarily wins by LTP, but playback 1 remains enabled and must not be auto-switched off.
   - **Expect on release:** The transient red contribution disappears and playback 1's blue returns without a default-color frame.
5. Configure playback 2's fader as **Temp**. Raise it from zero and then return it to zero.
   - **Expect:** While the Temp fader contributes, red wins according to its temporary level and LTP rules; playback 1 remains enabled. Returning the fader to zero removes the temporary contribution and restores blue.
6. Configure a playback 2 button as a toggled **TEMP** action. Toggle it on and then off.
   - **UI/schema capability required:** The current playback button action list includes Flash but not Temp. Once Temp-button assignment exists, it must have the same non-destructive arbitration and restoration semantics as the Temp fader, differing only in its toggled on/off interaction.

**Assertions:** Automatic switch-off occurs only when a non-temporary, fully raised, newer playback contribution covers every active attribute address of a playback whose automatic-off option is enabled. Partial overwrites, disabled automatic-off, Flash, Temp fader, and Temp button do not release the underlying playback. Removing a transient source reveals the exact underlying value immediately.

**Pass condition:** Normal full LTP replacement cleans up obsolete playbacks, while operator-configured persistence and every Flash/Temp path remain reversible and never destroy the scene they temporarily cover.

## CMD-002 — Set and synchronize speed groups from the command line

**Priority:** P1

**Primary layer:** Playwright E2E plus server integration

**Implementation status:** Specification only. Do not mark this scenario implemented until the `SPD GRP` command and synchronization behavior exist.

**Starting show:** Load canonical `default-stage.show`, immediately Save As `cmd-002-speed-groups.show`, and use only the active working copy. Start with no hardware connected, an empty command line, and Speed Groups A through E visible.

**Detailed procedure:**

1. Press `[SHIFT]`, then `[TIME]`.
   - **Expect:** The command line contains exactly `SPD GRP`.
2. Press `[1] [+] [1] [2] [0] [ENTER]`.
   - **Expect:** Speed Group A changes to exactly 120 BPM. Speed Groups B through E do not change.
3. Press `[SHIFT] [TIME] [2] [+] [1] [2] [7] [ . ] [5]` and inspect the command before pressing `[ENTER]`.
   - **Expect:** The command line reads `SPD GRP 2 + 127,5`; the decimal separator is displayed as a comma. Press `[ENTER]` and confirm Speed Group B reports exactly 127.5 BPM while A and C–E do not change.
4. Repeat the whole-number command for group numbers `3`, `4`, and `5`, using distinct BPM values. Confirm that the changed controls are Speed Groups C, D, and E respectively. No command may update a neighboring group.
5. Set Speed Group A to 120 BPM and Speed Group C to a different BPM. Press `[SHIFT] [TIME] [1] [AT] [3]` and inspect the command before pressing `[ENTER]`.
   - **Expect:** The command line reads `SPD GRP 1 AT 3`. Press `[ENTER]`; Speed Group A's 120 BPM is copied to Speed Group C, and their beat phases become synchronized. Observe multiple beats or advance application time through multiple beat boundaries and confirm their beat indicators remain aligned.
6. Set Speed Group C directly to 90 BPM with `[SHIFT] [TIME] [3] [+] [9] [0] [ENTER]`.
   - **Expect:** C changes to 90 BPM, A remains at 120 BPM, and the A–C synchronization is removed. A subsequent tempo change to either group does not change or re-synchronize the other.
7. Enter `[SHIFT] [TIME] [1] [AT] [3] [ENTER]` again and confirm A and C are synchronized at A's current BPM. Tap Speed Group A at a deliberately different tempo, using enough taps for the normal tap-tempo calculation.
   - **Expect:** A adopts the tapped BPM, C retains the BPM it had while linked, and the A–C synchronization is removed. Subsequent beats and speed changes are independent.

**Assertions:** The visible command text is exact; group numbers `1–5` map only to A–E; integer and decimal-comma BPM values retain their entered precision; `[AT]` copies the first group's BPM to the second and synchronizes their beat phase; direct BPM entry or tap tempo on either synchronized group removes that synchronization without changing the other group.

**Pass condition:** An operator can address, set, copy, and synchronize every speed group through `SPD GRP`, and either documented manual speed action reliably returns the affected groups to independent operation.

## Follow-ups

| Scenario | Next tests after the primary case | First failure checks |
| --- | --- | --- |
| CUE-001 | Add preset references, group membership edits, and save/restart. | Compare stored cue deltas, reconstructed tracked state, and active contribution order. |
| CUE-002 | Put cue-only changes at the first/last cue and combine them with tracked group references. | Inspect the generated restoration delta before testing navigation. |
| CUE-003 | Repeat with GO from OSC and REST, and with a non-zero cue delay. | Compare virtual timestamps and playback transition state at the first wrong checkpoint. |
| CUE-004 | Add split up/down intensity times, per-attribute delays, and Cue master fades across Color, Position, and Beam. | Compare stored per-value timing, Cue fallback timing, Force Cue Timing, and exact endpoints independently. |
| CUE-005 | Add FOLLOW/TIME chains, loops, disabled automatic triggers, and speed changes before the deadline. | Inspect preceding-Cue completion, scheduled application-time deadline, trigger ownership, and transition count. |
| CUE-006 | Change pages, select an unassigned playback, and repeat with explicit page/playback addresses. | Compare the stored active-playback identity with the Cuelist resolved by Shift-4 and Record. |
| CUE-007 | Repeat with Color and Position zero/default values and insert more than one Cue between tracking blocks. | Inspect stored zero deltas before blaming playback reconstruction. |
| CUE-008 | Record additional Cues in Preload and combine pending programmer changes with an already-running unrelated playback. | Compare persisted Cue data separately from preload and playback runtime state. |
| CUE-009 | Move and copy within one Cuelist, across Cuelists, and through page-playback addresses. | Compare the source Cue delta, reconstructed source status, and destination history before executing the operation. |
| CUE-010 | Repeat with Position and Beam attributes, per-attribute timing, and unequal priorities. | Dump contributions and LTP edit timestamps independently for every attribute address. |
| CUE-011 | Add multi-selection editing, large-list renumbering, undo/redo, and concurrent editor conflicts. | Separate local selected-row state from the one transactional Cuelist revision and playback runtime. |
| CUE-012 | Add longer Chasers, live Speed Group changes, Reset wrapping over several tracked attributes, restart after edits, and timing bypass across chained automatic Cues. | Inspect effective step duration, wrap/restart source reconstruction, timing-precedence flags, and migrated schema fields independently. |
| MIB-001 | Add Wrap Around look-ahead, multi-head fixtures, Blackout/Grand Master darkness, and competing future-position sources. | Inspect resolved-dark timestamps, target-Cue reconstruction, priority ownership, and cancellation reason before raw DMX. |
| CUE-013 | Delete first/last active Cues, edit neighboring Cues while output is held, and exercise Off/On restart modes from a deleted-active anchor. | Inspect persisted Cue list separately from held runtime contribution and previous/next navigation anchors. |
| MERGE-001 | Permute source creation/edit order and equal timestamps. | Dump normalized contributions with priority and edit timestamp before resolution. |
| MERGE-002 | Add group masters, blackout, and grand master. | Separate source arbitration from final intensity scaling and DMX encoding. |
| MERGE-003 | Cover multi-Cue source playbacks, partial fixture coverage, and multiple temporary overlays. | Inspect the exact covered-address set, auto-off option, master level, and temporary flags. |
| CMD-002 | Repeat synchronization with every source/target pair and decimal values near the accepted limits. | Compare visible command text, reported BPM precision, link state, and beat-phase origin. |
