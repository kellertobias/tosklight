# Playback Configuration

These scenarios specify the future Playback Configuration modal described in [`docs/planned features/10-playback-configuration.md`](../planned%20features/10-playback-configuration.md). They are specifications only: do not add or enable matching Playwright tests until the modal and authoritative playback-configuration model exist.

Every scenario loads the named canonical show, immediately uses Save As with the stated unique filename, and operates only on that working copy. The API and UI variants use independent copies and the shared normalized playback/output oracle. For UI cases, `[SET]` means the Lightning Desk Set key, not a browser modifier.

## PBK-001 — Set plus any playback control opens one inert configuration modal

**Priority:** P1  
**Primary layer:** Paired API/UI E2E

**Starting show:** Load canonical `compact-rig.show`, immediately Save As a separate `pbk-001-<surface>.show` copy for every entry surface, and assign a two-Cue Cuelist to page 1 playback 1.

**Detailed procedure:**

1. Put playback 1 on Cue 1 at a nonzero master level. Record its current Cue, output, fader value, button states, object revision, and audit tail.
2. Press `[SET]`, then independently touch the top, middle, and bottom buttons, the fader handle/track, and the software playback representation.
3. For every surface, confirm exactly one **Playback Configuration** modal opens for page 1 playback 1 with **Playback Function** and **Playback Layout** tabs.
4. Confirm the touched button did not execute, the Cuelist did not advance or release, and touching/dragging the fader while Set was armed did not change its value or output.
5. Repeat against an empty playback and confirm its modal opens without creating or assigning anything.
6. Cancel every modal and compare the complete playback state, show revision, event stream, and output with the pre-touch snapshot.

**Assertions:** Every constituent control resolves to the same playback identity while Set is armed. Set intercepts normal control behavior, and Cancel is mutation-free.

**Pass condition:** An operator can reliably configure a playback from any of its controls without accidentally operating it.

## PBK-002 — Assign, color, persist, and clear every playback function

**Priority:** P1  
**Primary layer:** Paired API/UI E2E plus persistence

**Starting show:** Load canonical `default-stage.show`, immediately Save As `pbk-002-functions.show`, create Cuelist 1 and Group 1, and retain distinct current values for Speed Groups A–E, Programmer Fade, Cue Fade, and Grand Master.

**Detailed procedure:**

1. Open an empty playback's configuration and, in independent copies, assign each function: Cuelist 1, Group Master 1, a Speed Master targeting each Speed Group A–E, Programmer Fade, Cue Fade, and Grand Master.
2. Confirm each assignment offers only its compatible button and fader choices. A one-button, two-button, three-button, faderless, and normal fader playback exposes exactly the controls physically present.
3. Choose a non-default swatch from the compact prominent-color palette. Confirm the same persisted color appears on all hardware button LEDs belonging to that playback and on its software representation.
4. Save and reload. Confirm target identity, function, compatible layout, color, and feedback survive exactly.
5. Change the assignment family and confirm incompatible prior mappings are replaced with the new type defaults rather than retained invisibly.
6. Activate a Cuelist playback, open configuration, choose **Clear Playback**, and cancel the confirmation once. Confirm nothing changes. Confirm on the second attempt.
7. Confirm the playback contribution is released, the slot is empty with default settings, and the referenced Cuelist still exists unchanged in the Cuelist Pool.

**Assertions:** Assignment is revision-checked and atomic. The compact palette needs no arbitrary color picker. Clear affects the playback slot, not the referenced show object.

**Pass condition:** Every supported playback function can be assigned, recognized, colored, persisted, changed safely, and cleared without deleting its source object.

## PBK-003 — Cuelist button mappings execute their exact actions

**Priority:** P1  
**Primary layer:** Paired API/UI E2E

**Starting show:** Load canonical `compact-rig.show`, immediately Save As a separate `pbk-003-<action>.show` copy per action, and assign a three-Cue tracked Cuelist to a three-button playback with a fader.

**Detailed procedure:**

1. Confirm the default layout is Top = Go minus, Middle = Go plus, Bottom = Flash, and Fader = Master.
2. Assign and invoke **Go plus** and **Go minus** independently. Confirm they move one Cue in the named direction using the stored transition timing.
3. Assign **Fast forward** and **Fast rewind**. From a marked virtual timestamp, confirm each moves one Cue in the named direction at the same application timestamp with no Cue/value delay or fade, without rewriting stored timing.
4. Assign **On**. With the physical fader below full, press On and confirm the virtual playback level becomes 100% and the Cuelist enters First or Continue according to its Restart setting.
5. Assign **Off**. Press it at a nonzero physical fader position and confirm output releases while the physical position remains recorded. Move the fader upward without first reaching zero and confirm the playback remains Off; take it fully to zero and then raise it to regain control.
6. Assign **Toggle** and confirm successive presses perform normal On then Off.
7. Assign **Select** and confirm it establishes the explicit active playback without changing Cue, level, or output.
8. Assign **Select contents**. Confirm the selection contains every fixture and live Group reference addressed anywhere in the Cuelist, in deterministic first-appearance order with duplicates removed, without executing a Cue or copying Cue values into the programmer.

**Assertions:** Each mapping is shown on the correct physical/software control and dispatches one action only. Button feedback, current/next Cue, playback API, audit events, and DMX agree.

**Pass condition:** Cuelist buttons are freely assignable within the supported action set and preserve the distinct navigation, activation, selection, and selection-content meanings.

## PBK-004 — Master, X-fade, and Temp faders retain distinct ownership

**Priority:** P1  
**Primary layer:** Playback integration plus paired UI E2E

**Starting show:** Load canonical `compact-rig.show`, immediately Save As a separate `pbk-004-<mode>.show` copy for each fader mode, and assign a three-Cue Cuelist with visible intensity and color changes to playback 1.

**Detailed procedure:**

1. With **Master**, move the fader through 0%, 50%, and 100%. Confirm it scales only playback intensity continuously and does not advance Cues.
2. With **X-fade**, begin on Cue 1 at one end. Move to 25%, 50%, 75%, and the opposite end and compare exact interpolated values with manual progress rather than elapsed Cue time. At the opposite end Cue 2 becomes current.
3. Attempt to continue in the same direction and confirm no extra Cue fires. Reverse through the same checkpoints to the original end; Cue 3 becomes current. Stored Cue timings remain byte-for-byte unchanged.
4. With **Temp**, keep another playback active beneath playback 1. Raise the Temp fader through partial and full levels, then return it to zero. Confirm it creates and removes a continuously variable temporary contribution while the underlying playback remains active and resumes without retriggering.
5. Save/reload each fader assignment and confirm mode, required travel direction, current/next state, and feedback restore deterministically.

**Assertions:** Master is an intensity scale, X-fade is bidirectional manual Cue progression, and Temp is a removable priority-stack contribution. None silently changes another mode's persisted data.

**Pass condition:** The same physical fader can serve each documented Cuelist role without conflating scaling, progression, or temporary ownership.

## PBK-005 — Flash, Temp, Swap, and protection preserve the LTP stack

**Priority:** P0  
**Primary layer:** Playback arbitration integration plus paired UI E2E

**Starting show:** Load canonical `default-stage.show`, immediately Save As a separate `pbk-005-<case>.show` copy per case, and create competing playbacks whose Cues address the same fixtures with different Intensity, Color, and Position values.

**Detailed procedure:**

1. Activate playback A, then hold **Flash** on playback B so B wins the overlapping HTP/LTP attributes. Confirm A is not automatically switched Off even when B temporarily overwrites every attribute. Release Flash and confirm A's prior state returns without GO, restart, or a default frame.
2. Repeat with **Temp**: one press adds B's temporary contribution and a second press removes it. Confirm the same restoration and no automatic Off of A.
3. Test both persisted Flash-release modes. **Release all** switches/releases B and removes every flashed contribution on release. **Release intensity only** leaves B active at zero intensity and retains B's applicable non-intensity Color and Position state under normal tracking/arbitration; confirm this is visible as normal persistent playback state rather than an orphan temporary entry.
4. In independent copies, enable and disable **Switch Cuelist off when fully overwritten** on A and repeat MERGE-003's normal full-overwrite case. Confirm only a complete non-temporary overwrite can switch A Off; partial overwrite, Flash, and Temp cannot. Save/reload the option.
5. Assign **Swap** to B. While held, confirm unprotected playbacks A and C are forced to zero without changing their Cue positions or stored fader levels; release and confirm their prior output returns without retriggering.
6. Enable **Protect from Swap** on A and repeat. A remains live, C is temporarily forced to zero, and B still behaves as Flash. Save/reload protection and repeat once.

**Assertions:** Inspect playback On/Off state separately from resolved output level and LTP ownership. Temporary contributions and Swap suppression have explicit lifetimes and never masquerade as permanent playback state changes.

**Pass condition:** Flash and Temp temporarily win arbitration without destroying underlying playback ownership, while Swap suppresses only unprotected playbacks and restores them exactly.

## PBK-006 — Master-specific layouts control the authoritative target

**Priority:** P1  
**Primary layer:** Paired API/UI E2E plus exact virtual time

**Starting show:** Load canonical `default-stage.show`, immediately Save As a separate `pbk-006-<function>.show` copy for every function below.

**Detailed procedure:**

1. **Speed Master:** target Speed Group A learned at 120 BPM. Confirm the three-button default is Top = Double, Middle = Half, Bottom = Learn; assign Pause as an alternate and verify all four actions affect the same Speed Group and Chaser phase feedback.
2. Test Direct BPM fader checkpoints: 0% = 0 BPM, 50% = 150 BPM, and 100% = 300 BPM. Test Learned-speed percentage: 0% = paused, 50% = 60 BPM, and 100% = 120 BPM. For Centered relative, confirm 50% = exactly 120 BPM, below 50% is slower, and above 50% is faster. Add exact endpoints once its persisted multiplier range/curve is fixed.
3. Repeat assignment for Speed Groups B–E and confirm no neighboring group changes. Command-line/tap-tempo controls and every playback surface show the same rate and phase.
4. **Group Master:** confirm its fader is fixed to the selected Group master. Verify Select retains a live Group reference, Select dereferenced produces individual fixture members, and Flash temporarily brings the master to full without changing its stored fader level.
5. **Grand Master:** confirm its fader is fixed to Grand Master. Verify Blackout toggles global blackout, Flash temporarily brings the master to full without changing its stored level, and Pause Dynamics freezes/resumes Dynamics phase without deleting or resetting it.
6. **Programmer Fade and Cue Fade:** confirm each fader controls the matching existing time master with identical range, unit, and feedback. Every available button is visibly disabled and produces no command, event, or output change.

**Assertions:** Each specialized layout exposes only valid controls and updates its one authoritative system target. Persisted mappings and values agree across UI, hardware feedback, API state, and playback behavior.

**Pass condition:** Speed, Group, Grand Master, Programmer Fade, and Cue Fade assignments behave as real views of existing master state rather than disconnected playback-local copies.

## Failure follow-ups

| Scenario | Expand with | Diagnose first |
| --- | --- | --- |
| PBK-001 | Page changes, touch cancellation, and every hardware topology. | Set interception, page/playback identity, and accidental action/fader events. |
| PBK-002 | Concurrent edits, invalid deleted targets, and legacy-show migration. | Revision conflict handling and partial persisted mutations. |
| PBK-003 | Wrap/Restart combinations and absent Cues/fixtures. | Dispatched action verb and target playback identity. |
| PBK-004 | Reversal mid-X-fade and competing programmer values. | Manual progress, direction latch, and temporary-source ownership. |
| PBK-005 | Several protected playbacks and nested temporary actions. | Permanent On/Off state versus temporary suppression/LTP entries. |
| PBK-006 | Fractional learned BPM and paused Chasers. | Shared Speed Group/master identity and virtual phase clock. |
