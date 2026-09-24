# Dynamic Reuse Across Groups

## Purpose

Prove that a Dynamic authored with nothing selected stays fixture-independent. Applied to one
Group and recorded, it runs on that Group only. Applied again to another Group and recorded
separately, it runs there on its own, and the stored Dynamic stays bound to neither Group.

Fixtures used throughout: eight `ROBE › Robin 300 LEDWash` movers in mode `Mode 3`.

- Group 1 holds movers 1–4.
- Group 2 holds movers 5–8.
- Position preset 1 (**Down**) is pan 40 % and tilt 20 % on all eight movers.
- Position preset 2 (**Up**) is pan 60 % and tilt 80 % on all eight movers.

## BENCH-DYNAMIC-REUSE-001 — One Dynamic, two Groups, two Virtual Playbacks

This scenario is automated in `tests/114-semantic-dynamic-reuse-across-groups.spec.ts`.

1. With nothing selected, create Dynamic 1, **Rise**. Confirm the stored Dynamic is targetless: it
   names no Group and no fixtures. Rise has:
   - one 4-second cycle, looping;
   - grid phase spread;
   - an intensity lane from 0 to full;
   - pan and tilt lanes from **Down** to **Up**.
2. Select Group 1, apply Rise, and record it onto Playback 1 (**Rise 1**). Clear the programmer.
3. Select Group 2, apply Rise, and record it onto Playback 2 (**Rise 2**). Clear the programmer.
4. Confirm Rise is still targetless. Give each Playback a single Toggle button.
5. Assign both Playbacks to Virtual Playback cells 1 and 2.
6. Turn Rise 1 on. A quarter-cycle later, movers 1–4 are each a quarter-cycle apart in selection
   order, and movers 5–8 stay dark. With no stage plan, the grid spread falls back to selection
   order. At that moment the movers read as follows:

   | Mover | Intensity | Position between Down and Up |
   |---|---|---|
   | 1 | 25 % | a quarter of the way |
   | 2 | 50 % | halfway |
   | 3 | 75 % | three-quarters of the way |
   | 4 | 0 % | at Down (the cycle wrapped) |

7. Turn Rise 2 on. Movers 5–8 start their own rise from the same phases. Movers 1–4 carry on,
   a quarter-cycle further.
8. Turn Rise 1 off. Movers 1–4 go dark while movers 5–8 keep rising.
9. Turn Rise 2 off. All eight movers are dark, and Rise is still targetless.
