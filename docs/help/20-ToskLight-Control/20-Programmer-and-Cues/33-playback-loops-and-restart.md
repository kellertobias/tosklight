# Playback loops and restart

Each Cue row can define a **Jump** destination and positive **Jump Count**. The destination is tied to the Cue itself, so the displayed Cue number and name follow later renumbering. On each arrival at that jump point, the next GO takes the Jump destination until the configured count is exhausted; the following GO continues to the ordinary next Cue. Jump points count independently. Turning the Cuelist Off or restarting it clears all jump counts. Fast-forward or Go To navigation that passes a jump point clears that point's count without taking its Jump, so a later run cannot inherit a stale loop.

## Restart behavior

First and Continue policies determine how a Cuelist starts after release or restart. Looping and chaser modes change end-of-list behavior. Test the exact production policy after a real server/app restart.

Use [Cue triggers](13-triggers-chasers-and-speed.md) for FOLLOW, TIME, Link, and Chaser behavior.
