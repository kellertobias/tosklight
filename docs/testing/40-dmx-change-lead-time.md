# DMX change lead time

## Purpose and status

These scenarios are the acceptance contract for the change lead time in the DMX output pane's
output summary (TL-659). The operator contract is
[Utility and diagnostics › DMX output](../help/10-Desk/30-Windows/04-utility-and-diagnostics.md#dmx-output).

The change lead time is the longest time from the instant a Cue or Dynamic **should** start
outputting to the instant the output frame that first carries it was **sent**. The start instant
is the GO, Back, Goto or Playback On for a Cue transition, the tick that takes an automatic Follow,
Wait, Link or Chaser step, and for a Dynamic its activation, its activation plus its delay, or the
beat or bar boundary a boundary start waits for. A frame carries every start due by its sample
instant and is measured from the earliest of them once the frame has been sent. A frame that is
never sent hands its starts to the next one that is.

The measurement is constant-time atomic work on the output lane: no lock and no allocation. A
lead longer than five seconds is a running source re-established on its original start instant,
not a start reaching output late; it is counted as excluded instead of as the maximum.

Executable coverage:

- Rust: `crates/light/domain/output/src/change_lead.rs` (maximum, recent window, carry-over,
  exclusion and reset on a deterministic clock), `crates/shared/core/src/change_lead.rs` (the
  start ledger), `crates/light/domain/playback/src/tests/change_lead.rs` (Cue transitions) and
  `crates/light/domain/dynamics/src/tests/change_lead.rs` (Start Now, delayed and boundary starts).
- Vitest: `apps/light-desktop/src/windows/DmxWindowView.test.tsx`.
- Root Playwright: `tests/129-dmx-change-lead-time.spec.ts`.

Rig: the `compact-rig` show with fixtures 1–4 in Group 1, and Playback 1 running a two-Cue looped
Cuelist that sets Group 1 to Full and to 50 % without fade. The test bench freezes the desk clock,
so a frame rendered _N_ ms after a GO measures exactly _N_ ms.

## CHANGE-LEAD-001 — The longest lead is reported, and the operator resets it

1. Reset the change lead time. Max, Last 60 s and Latest read **—**; no start has been measured.
2. Press GO on Playback 1 and let the next output frame leave 40 ms later. Fixtures 1–4 output
   255. Max, Last 60 s and Latest read **40.0 ms**; one frame carried a start.
3. Press GO again and let the next frame leave 15 ms later, then send another frame with no GO.
   Max and Last 60 s stay at **40.0 ms**; Latest reads **15.0 ms**. A frame without a start
   measures nothing.
4. Let 61 seconds pass. Last 60 s reads **—**; Max still reads **40.0 ms** until a reset.
5. Reset. Every reading returns to **—**.
6. Press GO and send a frame 20 ms later, then resend the step 5 reset with its original request
   identity. The desk answers with the first outcome and does not reset again: Max reads
   **20.0 ms**.

## CHANGE-LEAD-002 — The DMX output summary shows and resets the change lead time

1. Open a DMX output pane with no slot selected. Below **Errors**, the **Change lead time** section
   shows **Max**, **Last 60 s** and **Latest** as **—**, and a **Reset** button that is a desk-sized
   touch target, at least 40 px high.
2. Press GO on Playback 1 and send the next frame 30 ms later. Within a second, all three readings
   show **30.0 ms**.
3. Tap **Reset**. All three readings return to **—**, and the desk reports no measured start.
4. A failed reset leaves the last readings in place and shows the error under the readings.
