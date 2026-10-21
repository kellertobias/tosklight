# Physics-driven Scenery

Compatible scenery packages respond to their documented DMX **Hold**, **Release** and **Reset**
values. Release is latched: once a body falls, a DMX interruption or reconnect does not put it back.

Use its explicit Reset value, or **Running & Output > Visualizer > Reset physics scenery**, to
return it to the authored pose. Selecting another view is not a reset. Check the selected renderer
target before sending the reset.

The shipped **Generic Kabuki Curtain** uses one slot: `0–31` Reset, `32–191` Hold and `192–255`
Release. These values apply only to that Generic profile; use the manufacturer's chart for real
hardware.

Gravity and floor settling are available in every quality. High and Ultra can also include
configured scenery collisions. The renderer retains released or settled bodies across a restart;
Reset clears that retained state. Timecode seeking reconstructs the applicable Release/Reset history.
A faulty package is diagnosed for that body without stopping the rest of the rig.
